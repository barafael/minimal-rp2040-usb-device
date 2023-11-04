#[derive(Debug)]
pub struct Formatter<const N: usize> {
    data: [u8; N],
    cursor: usize,
}

impl<const N: usize> Formatter<N> {
    pub const fn new() -> Self {
        Self {
            data: [0; N],
            cursor: 0,
        }
    }

    #[inline]
    pub const fn into_array(self) -> [u8; N] {
        self.data
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.cursor
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.data[0..self.cursor]).unwrap()
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        self.data.len()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.cursor = 0;
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.cursor == 0
    }

    #[inline]
    pub const fn full(&self) -> bool {
        self.capacity() == self.cursor
    }
}

impl<const N: usize> core::fmt::Write for Formatter<N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let rest_len = N - self.cursor;
        let len = if rest_len < s.len() {
            rest_len
        } else {
            s.len()
        };
        self.data[self.cursor..(self.cursor + len)].copy_from_slice(&s.as_bytes()[0..len]);
        self.cursor += len;
        Ok(())
    }
}
