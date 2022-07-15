const PAGE_SIZE: usize = 4096; // 4Kb
const HEADER_SIZE: usize = 32; // 32 bytes

#[derive(Debug, PartialEq)]
pub struct PageId(usize);

#[derive(Debug)]
pub enum PageType {
    KeyPage = 0,
    KeyValuePage = 1,
}

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
    fn empty() -> Self {
        let mut s = Self {
            data: [0; PAGE_SIZE],
        };
        s.write_magic_number();
        s.write_lower_offset(0);
        s.write_upper_offset((PAGE_SIZE-HEADER_SIZE) as u16);
        s.write_overflow_page(None);
        s.write_flags(0);
        s
    }

    fn write_magic_number(&mut self) {
        self.data[0..4].copy_from_slice(b"PAGE");
    }

    fn write_lower_offset(&mut self, offset: u16) {
        self.data[4..6].copy_from_slice(&offset.to_le_bytes());
    }

    fn read_lower_offset(&self) -> u16 {
        u16::from_le_bytes(self.data[4..6].try_into().unwrap())
    }

    fn write_upper_offset(&mut self, offset: u16) {
        self.data[6..8].copy_from_slice(&offset.to_le_bytes());
    }

    fn read_upper_offset(&self) -> u16 {
        u16::from_le_bytes(self.data[6..8].try_into().unwrap())
    }

    fn write_overflow_page(&mut self, page: Option<PageId>) {
        match page {
            Some(p) => {
                self.data[8..16].copy_from_slice(&p.0.to_le_bytes())
            }
            None => {
                self.data[8..16].copy_from_slice(&0usize.to_le_bytes())
            }
        }
    }

    fn read_overflow_page(&self) -> Option<PageId> {
        let pid = usize::from_le_bytes(self.data[8..16].try_into().unwrap());
        if pid == 0 {
            None
        } else {
            Some(PageId(pid))
        }
    }

    fn write_flags(&mut self, offset: u16) {
        self.data[16..18].copy_from_slice(&offset.to_le_bytes());
    }

    fn read_flags(&self) -> u16 {
        u16::from_le_bytes(self.data[16..18].try_into().unwrap())
    }

    fn write_cell_data(&mut self, from_offset: u16, data: &[u8]) {
        let idx = HEADER_SIZE + from_offset as usize;
        let idx_to = idx + data.len();
        self.data[idx..idx_to].copy_from_slice(data);
    }

    fn read_cell(&self, from_offset: u16, len: u16) -> &[u8] {
        let idx = HEADER_SIZE + from_offset as usize;
        let idx_to = idx + len as usize;
        &self.data[idx..idx_to]
    }

    // 1 pointer is 4 bytes
    // 2 for offset and 2 for len
    fn write_pointer(&mut self, from_offset: u16, addr: u16, len: u16) {
        let idx = HEADER_SIZE + from_offset as usize;
        self.data[idx..idx + 2].copy_from_slice(&addr.to_le_bytes());
        self.data[idx + 2..idx + 4].copy_from_slice(&len.to_le_bytes());
    }

    fn read_pointer(&self, from_offset: u16) -> (u16, u16) {
        // if read from non-ptr range
        if from_offset > self.read_lower_offset() {
            panic!("Not a pointer");
        }
        let idx = HEADER_SIZE + from_offset as usize;
        let addr = u16::from_le_bytes(self.data[idx..idx+2].try_into().unwrap());
        let len = u16::from_le_bytes(self.data[idx+2..idx+4].try_into().unwrap());

        (addr, len)
    }

    fn add_cell(&mut self, data: &[u8]) {
        let lower_offset = self.read_lower_offset();
        let upper_offset = self.read_upper_offset();
        if data.len() > (upper_offset - lower_offset) as usize {
            panic!("Overflow page");
        }
        let addr = upper_offset - data.len() as u16;
        self.write_cell_data(addr, data);
        self.write_pointer(lower_offset, addr as u16, data.len() as u16);
        let new_lower = lower_offset + 4;
        let new_upper = addr;
        self.write_lower_offset(new_lower);
        self.write_upper_offset(new_upper);
    }

    fn read_nth_cell(&self, nth: usize) -> &[u8] {
        let at = (nth * 4) as u16;
        let (ptr, len) = self.read_pointer(at);
        self.read_cell(ptr, len)
    }

    fn cells_count(&self) -> u16 {
        self.read_lower_offset() / 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let mut page = Page::empty();
        assert_eq!(page.read_lower_offset(), 0);
        assert_eq!(page.read_upper_offset(), (PAGE_SIZE - HEADER_SIZE) as u16);
        assert_eq!(page.read_overflow_page(), None);
        assert_eq!(page.read_flags(), 0);

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
