// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[derive(Copy, Clone, Default)]
pub enum LineEndings {
    #[default]
    Lf, // Unix-style line-endings
    CrLf, // MS-DOS style line-endings
}

impl LineEndings {
    /// Some user-visible name to display in the title bar, if not default
    pub fn name(&self) -> Option<&'static str> {
        match self {
            Self::Lf => None,
            Self::CrLf => Some("DOS"),
        }
    }

    /// Create rope from reader, probes for its line endings
    /// and converts to Unix-style if necessary.
    pub fn reader_to_rope<R>(r: R) -> std::io::Result<(Self, ropey::Rope)>
    where
        R: std::io::Read,
    {
        use std::io::{BufRead, BufReader};

        let mut rope = ropey::RopeBuilder::default();
        let mut reader = BufReader::new(r);
        let mut line = String::default();

        // probe the file's first line for its ending, if any
        reader.read_line(&mut line)?;
        let endings = match line.ends_with("\r\n") {
            true => LineEndings::CrLf,
            false => LineEndings::Lf,
        };

        // transfer all lines from reader to rope,
        // converting \r\n to \n if necessary
        while !line.is_empty() {
            // Just because the first line ends with \r\n
            // doesn't mean we can assume all of them do (or the reverse).
            // For mixed-endings files, we update them to be consistent
            // with the first line's endings,
            // which appears to be how Nano does things.
            if line.ends_with("\r\n") {
                assert_eq!(line.pop(), Some('\n'));
                assert_eq!(line.pop(), Some('\r'));
                line.push('\n');
            }
            rope.append(&line);
            line.clear();
            reader.read_line(&mut line)?;
        }

        Ok((endings, rope.finish()))
    }

    /// Reads string from reader using our line endings,
    /// converting to Unix-style if necessary.
    pub fn reader_to_string<R>(self, mut r: R) -> std::io::Result<String>
    where
        R: std::io::Read,
    {
        let mut s = String::new();
        r.read_to_string(&mut s)?;
        match self {
            Self::Lf => Ok(s),
            Self::CrLf => Ok(s.replace("\r\n", "\n")),
        }
    }

    /// Writes rope to writer using our line endings,
    /// converting from Unix-style if necessary.
    pub fn rope_to_writer<W>(self, rope: &ropey::Rope, mut w: W) -> std::io::Result<()>
    where
        W: std::io::Write,
    {
        match self {
            Self::Lf => rope.write_to(w),
            Self::CrLf => rope.lines().try_for_each(|line| {
                let line = std::borrow::Cow::from(line);
                match line.strip_suffix('\n') {
                    Some(line) => write!(w, "{line}\r\n")?,
                    None => write!(w, "{line}")?,
                }
                Ok(())
            }),
        }
    }
}
