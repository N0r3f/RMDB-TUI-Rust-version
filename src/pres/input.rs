use std::io::{self, Read};

pub enum Key {
    Up,
    Down,
    Enter,
    Quit,
    Backspace,
    Char(char),
    Unknown,
}

pub struct InputReader;

impl InputReader {
    pub fn new() -> Self {
        Self
    }

    pub fn read_key(&self) -> Result<Key, io::Error> {
        let mut stdin = io::stdin();
        let mut first = [0u8; 1];
        
        stdin.read_exact(&mut first)?;
        
        if first[0] == 0x1b {
            let mut second = [0u8; 1];
            match stdin.read_exact(&mut second) {
                Ok(_) if second[0] == b'[' => {
                    let mut third = [0u8; 1];
                    match stdin.read_exact(&mut third) {
                        Ok(_) => {
                            match third[0] {
                                b'A' => return Ok(Key::Up),
                                b'B' => return Ok(Key::Down),
                                _ => return Ok(Key::Unknown),
                            }
                        }
                        Err(_) => return Ok(Key::Unknown),
                    }
                }
                _ => return Ok(Key::Unknown),
            }
        }
        
        match first[0] {
            b'\n' | b'\r' => Ok(Key::Enter),
            b'q' | 3 => Ok(Key::Quit),
            0x7f | 0x08 => Ok(Key::Backspace), // DEL / BS
            b' ' => Ok(Key::Char(' ')),
            c if c.is_ascii_alphanumeric() || c.is_ascii_punctuation() => {
                Ok(Key::Char(c as char))
            }
            _ => Ok(Key::Unknown),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_reader_creation() {
        let _reader = InputReader::new();
        assert!(true);
    }
}
