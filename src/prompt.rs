#[derive(Default)]
pub struct Prompt {
    value: Vec<char>,
}

impl Prompt {
    pub fn push(&mut self, c: char) {
        self.value.push(c)
    }

    pub fn pop(&mut self) -> Option<char> {
        self.value.pop()
    }

    pub fn set(&mut self, value: &[char]) {
        self.value.clear();
        self.value.extend(value);
    }

    pub fn chars(&self) -> &[char] {
        self.value.as_slice()
    }
}

impl std::fmt::Display for Prompt {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.value.iter().try_for_each(|c| c.fmt(f))
    }
}
