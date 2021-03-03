const BUFFER_SIZE: u16 = 128;

pub struct RingBuffer<T>
where
    T: Default + Copy,
{
    head: u16,
    tail: u16,
    contents: [T; BUFFER_SIZE as usize],
}

impl<T> RingBuffer<T>
where
    T: Default + Copy,
{
    pub fn new() -> RingBuffer<T> {
        RingBuffer {
            head: 0,
            tail: 0,
            contents: [T::default(); BUFFER_SIZE as usize],
        }
    }

    pub fn clear(&mut self) {
        self.tail = 0;
        self.head = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    pub fn size(&self) -> u16 {
        if self.tail >= self.head {
            self.tail - self.head
        } else {
            (BUFFER_SIZE - self.head) + self.tail
        }
    }

    pub fn push(&mut self, c: T) {
        let buf_ref = self
            .contents
            .get_mut(usize::from(self.tail % BUFFER_SIZE))
            .unwrap();
        *buf_ref = c;
        self.tail = (self.tail + 1) % BUFFER_SIZE;

        // we've hit the maximum dimensions of the buffer
        // we'll advance the head and overwrite
        if self.tail == self.head {
            self.head = (self.head + 1) % BUFFER_SIZE;
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            match self.contents.get(usize::from(self.head % BUFFER_SIZE)) {
                Some(t) => {
                    self.head = (self.head + 1) % BUFFER_SIZE;
                    Some(*t)
                }
                None => None,
            }
        }
    }
}

impl<T> Default for RingBuffer<T>
where
    T: Default + Copy,
{
    fn default() -> Self {
        RingBuffer::new()
    }
}

impl<'t, T> Iterator for RingBuffer<T>
where
    T: Default + Copy,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.pop()
    }
}
