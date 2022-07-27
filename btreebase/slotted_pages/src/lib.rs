use std::num::NonZeroU64;

use binary_layout::{define_layout, LayoutAs, FieldSliceAccess};

const PAGE_SIZE: usize = 4096; // 4Kb

#[derive(Debug, PartialEq)]
pub struct PageId(NonZeroU64);

#[derive(Debug, PartialEq)]
pub struct MaybePageId(u64);

impl LayoutAs<u64> for MaybePageId {
    fn read(v: u64) -> Self {
        MaybePageId(v)
    }

    fn write(v: Self) -> u64 {
        v.0
    }
}

impl MaybePageId {
    pub fn from_page_id(p: Option<PageId>) -> Self {
        MaybePageId(p.map(|page_id| page_id.0.get()).unwrap_or(0))
    }

    pub fn to_page_id(self) -> Option<PageId> {
        NonZeroU64::new(self.0).map(PageId)
    }
}

#[derive(Debug)]
pub enum PageType {
    KeyPage = 0,
    KeyValuePage = 1,
}

define_layout!(page_header, LittleEndian, {
    magic: [u8; 4],
    lower_offset: u16,
    upper_offset: u16,
    overflow_page: MaybePageId as u64,
    flags: u16,
});
const HEADER_SIZE: usize = binary_layout::internal::unwrap_field_size(page_header::SIZE);

define_layout!(page, LittleEndian, {
    header: page_header::NestedView,
    body: [u8; PAGE_SIZE - HEADER_SIZE],
});

/// Datastructure to store a single unit of fixed-size data in disk
/// Structure:
/// MAGIC NUMBER:
/// b"PAGE" or [50 41 47 45]
/// LOWER OFFSET
/// 2 bytes
/// UPPER OFFSET
/// 2 bytes
/// OVERFLOW PAGE
/// 4 bytes, 0 means None
/// PAGE_FLAG
/// 2 bytes
pub struct Page {
    data: [u8; PAGE_SIZE],
}

impl Page {
    fn header_mut_view(&mut self) -> page_header::View<impl AsRef<[u8]> + AsMut<[u8]> + '_> {
        let page_view = page::View::new(&mut self.data[..]);
        let header_view = page_view.into_header();

        header_view
    }

    fn header_view(&self) -> page_header::View<impl AsRef<[u8]> + '_> {
        let page_view = page::View::new(&self.data[..]);
        let header_view = page_view.into_header();

        header_view
    }

    fn body_mut(&mut self) -> &mut [u8] {
        page::body::data_mut(&mut self.data[..])
    }

    fn body(&self) -> &[u8] {
        page::body::data(&self.data[..])
    }

    fn empty() -> Self {
        let mut s = Self {
            data: [0; PAGE_SIZE],
        };
        s.header_mut_view().magic_mut().copy_from_slice(b"PAGE");
        s.header_mut_view().lower_offset_mut().write(0);
        s.header_mut_view().upper_offset_mut().write((PAGE_SIZE-HEADER_SIZE) as u16);
        s.header_mut_view().overflow_page_mut().write(MaybePageId::from_page_id(None));
        s.header_mut_view().flags_mut().write(0);
        s
    }

    fn write_cell_data(&mut self, from_offset: u16, data: &[u8]) {
        let idx = from_offset as usize;
        let idx_to = idx + data.len();
        self.body_mut()[idx..idx_to].copy_from_slice(data);
    }

    fn read_cell(&self, from_offset: u16, len: u16) -> &[u8] {
        let idx = from_offset as usize;
        let idx_to = idx + len as usize;
        &self.body()[idx..idx_to]
    }

    // 1 pointer is 4 bytes
    // 2 for offset and 2 for len
    fn write_pointer(&mut self, from_offset: u16, addr: u16, len: u16) {
        let idx = from_offset as usize;
        self.body_mut()[idx..idx + 2].copy_from_slice(&addr.to_le_bytes());
        self.body_mut()[idx + 2..idx + 4].copy_from_slice(&len.to_le_bytes());
    }

    fn read_pointer(&self, from_offset: u16) -> (u16, u16) {
        // if read from non-ptr range
        if from_offset > self.header_view().lower_offset().read() {
            panic!("Not a pointer");
        }
        let idx = from_offset as usize;
        let addr = u16::from_le_bytes(self.body()[idx..idx+2].try_into().unwrap());
        let len = u16::from_le_bytes(self.body()[idx+2..idx+4].try_into().unwrap());

        (addr, len)
    }

    fn add_cell(&mut self, data: &[u8]) {
        let lower_offset = self.header_view().lower_offset().read();
        let upper_offset = self.header_view().upper_offset().read();
        if data.len() > (upper_offset - lower_offset) as usize {
            panic!("Overflow page");
        }
        let addr = upper_offset - data.len() as u16;
        self.write_cell_data(addr, data);
        self.write_pointer(lower_offset, addr as u16, data.len() as u16);
        let new_lower = lower_offset + 4 as u16;
        let new_upper = addr as u16;
        self.header_mut_view().lower_offset_mut().write(new_lower);
        self.header_mut_view().upper_offset_mut().write(new_upper);
    }

    fn read_nth_cell(&self, nth: usize) -> &[u8] {
        let at = (nth * 4) as u16;
        let (ptr, len) = self.read_pointer(at);
        self.read_cell(ptr, len)
    }

    fn cells_count(&self) -> u16 {
        self.header_view().lower_offset().read() / 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let mut page = Page::empty();
        assert_eq!(page.header_view().lower_offset().read(), 0);
        assert_eq!(page.header_view().upper_offset().read(), (PAGE_SIZE - HEADER_SIZE) as u16);
        assert_eq!(page.header_view().overflow_page().read().to_page_id(), None);
        assert_eq!(page.header_view().flags().read(), 0);

        page.add_cell(b"Hello, World");
        page.add_cell(b"Cop");
        page.add_cell(b"Han Le");
        page.add_cell(b"Koujir");

        assert_eq!(page.read_nth_cell(0), b"Hello, World");
        assert_eq!(page.read_nth_cell(1), b"Cop");
        assert_eq!(page.read_nth_cell(2), b"Han Le");
        assert_eq!(page.read_nth_cell(3), b"Koujir");
        assert_eq!(page.cells_count(), 4);
    }
}
