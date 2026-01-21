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
enum ShellToken {
    #[token("break")]
    #[token("case")]
    #[token("continue")]
    #[token("do")]
    #[token("done")]
    #[token("elif")]
    #[token("else")]
    #[token("esac")]
    #[token("exit")]
    #[token("fi")]
    #[token("for")]
    #[token("function")]
    #[token("if")]
    #[token("in")]
    #[token("read")]
    #[token("return")]
    #[token("select")]
    #[token("shift")]
    #[token("then")]
    #[token("time")]
    #[token("until")]
    #[token("while")]
    #[token("declare")]
    #[token("eval")]
    #[token("exec")]
    #[token("export")]
    #[token("let")]
    #[token("local")]
    #[token("-eq")]
    #[token("-ne")]
    #[token("-gt")]
    #[token("-lt")]
    #[token("-ge")]
    #[token("-le")]
    #[token("-ef")]
    #[token("-ot")]
    #[token("-nt")]
    Keyword,
    #[token("awk")]
    #[token("cat")]
    #[token("cd")]
    #[token("chgrp")]
    #[token("chmod")]
    #[token("chown")]
    #[token("cp")]
    #[token("cut")]
    #[token("echo")]
    #[token("env")]
    #[token("grep")]
    #[token("head")]
    #[token("install")]
    #[token("ln")]
    #[token("make")]
    #[token("mkdir")]
    #[token("mv")]
    #[token("popd")]
    #[token("printf")]
    #[token("pushd")]
    #[token("rm")]
    #[token("rmdir")]
    #[token("sed")]
    #[token("set")]
    #[token("sort")]
    #[token("tail")]
    #[token("tar")]
    #[token("touch")]
    #[token("umask")]
    #[token("unset")]
    Command,
    #[regex("#.*", allow_greedy = true)]
    Comment,
    #[regex("(-[[:alpha:]]|--[[:alpha:]-]+)")]
    Option,
    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    String,
    #[regex("[[:lower:]][[:lower:][:digit:]_]*")]
    Variable,
}

impl TryFrom<ShellToken> for Color {
    type Error = ();

    fn try_from(t: ShellToken) -> Result<Color, ()> {
        match t {
            ShellToken::Keyword => Ok(Color::Green),
            ShellToken::Command => Ok(Color::LightBlue),
            ShellToken::Comment => Ok(Color::Cyan),
            ShellToken::Option => Ok(Color::LightMagenta),
            ShellToken::String => Ok(Color::Yellow),
            ShellToken::Variable => Err(()),
        }
    }
}

#[derive(Debug)]
pub struct Shell;

impl std::fmt::Display for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Shell".fmt(f)
    }
}

highlighter!(Shell, ShellToken);
