// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::highlighter;
use logos::Logos;
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum PerlToken {
    #[regex(
        "abs|accept|alarm|atan2|bin(d|mode)|bless|caller|ch(dir|mod|op|omp|own|r|root)|close(dir)?|connect|cos|crypt"
    )]
    #[regex(
        "dbm(close|open)|defined|delete|dump|each|eof|eval(bytes)?|exec|exists|exp|fc|fcntl|fileno|flock|fork|format|formline"
    )]
    #[regex(
        "get(c|login|peername|pgrp|ppid|priority|(gr|pw)nam|(host|net|proto|serv)byname|pwuid|grgid|(host|net)byaddr|protobynumber|servbyport)"
    )]
    #[regex(
        "([gs]et|end)(pw|gr|host|net|proto|serv)ent|getsock(name|opt)|glob|gmtime|grep|hex|import|index|int|ioctl|join"
    )]
    #[regex(
        "keys|kill|lc|lcfirst|length|link|listen|local(time)?|lock|log|lstat|map|mkdir|msg(ctl|get|snd|rcv)|oct"
    )]
    #[regex(
        "open(dir)?|ord|pack|pipe|pop|pos|printf?|prototype|push|q|qq|qr|qx|qw|quotemeta|rand|read(dir|line|link|pipe)?"
    )]
    #[regex(
        "recv|redo|ref|rename|require|reset|reverse|rewinddir|rindex|rmdir|say|scalar|seek(dir)?|select|sem(ctl|get|op)"
    )]
    #[regex(
        "send|set(pgrp|priority|sockopt)|shift|shm(ctl|get|read|write)|shutdown|sin|sleep|socket(pair)?|sort|splice|split"
    )]
    #[regex(
        "sprintf|sqrt|srand|state?|study|substr|symlink|sys(call|open|read|seek|tem|write)|tell(dir)?|tied?|times?|try?"
    )]
    #[regex(
        "truncate|uc|ucfirst|umask|un(def|link|pack|shift|tie)|utime|values|vec|wait(pid)?|wantarray|warn|write"
    )]
    Function,
    #[regex(
        "continue|die|do|else|elsif|exit|for(each)?|fork|goto|if|last|next|return|unless|until|while",
        priority = 5
    )]
    #[regex("and|cmp|eq|ge|gt|isa|le|lt|ne|not|or|x|xor")]
    #[regex("my|no|our|package|sub|use")]
    Flow,
    #[regex(r#"\$[[:alpha:]_][[:alnum:]_]*"#)]
    Variable,
    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    String,
    #[regex("[smy]?/.*?/")]
    Regex,
    #[regex("#.*", allow_greedy = true)]
    Comment,
}

impl TryFrom<PerlToken> for Color {
    type Error = ();

    fn try_from(t: PerlToken) -> Result<Color, ()> {
        match t {
            PerlToken::Function => Ok(Color::Red),
            PerlToken::Flow => Ok(Color::Magenta),
            PerlToken::Variable => Ok(Color::Cyan),
            PerlToken::String => Ok(Color::Yellow),
            PerlToken::Regex => Ok(Color::Blue),
            PerlToken::Comment => Ok(Color::Green),
        }
    }
}

#[derive(Debug)]
pub struct Perl;

impl std::fmt::Display for Perl {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Perl".fmt(f)
    }
}

highlighter!(Perl, PerlToken);
