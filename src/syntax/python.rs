use logos::Logos;
use ratatui::style::Color;

#[derive(Logos, Debug)]
#[logos(skip r"[ \t\n]+")]
enum PythonToken {
    #[regex("def [[:alnum:]_]+")]
    Function,
    #[token("and")]
    #[token("as")]
    #[token("assert")]
    #[token("async")]
    #[token("await")]
    #[token("break")]
    #[token("class")]
    #[token("continue")]
    #[token("def")]
    #[token("del")]
    #[token("elif")]
    #[token("else")]
    #[token("except")]
    #[token("finally")]
    #[token("for")]
    #[token("from")]
    #[token("global")]
    #[token("if")]
    #[token("import")]
    #[token("in")]
    #[token("is")]
    #[token("lambda")]
    #[token("nonlocal")]
    #[token("not")]
    #[token("or")]
    #[token("pass")]
    #[token("raise")]
    #[token("return")]
    #[token("try")]
    #[token("while")]
    #[token("with")]
    #[token("yield")]
    Keyword,
    #[token("True")]
    #[token("False")]
    #[token("None")]
    Literal,
    #[regex("@[[:alpha:]_][[:alnum:]_.]*")]
    Decorator,
    #[regex(r#"\"([^\\\"]|\\.)*\""#)]
    String,
    #[regex(r"'([^\\']|\\.)*'")]
    SingleQuotedString,
    #[regex("#.*", allow_greedy = true)]
    Comment,
    #[regex("[[:alpha:]_][[:alnum:]_.]*")]
    Variable,
}

impl TryFrom<PythonToken> for Color {
    type Error = ();

    fn try_from(t: PythonToken) -> Result<Color, ()> {
        match t {
            PythonToken::Function => Ok(Color::LightBlue),
            PythonToken::Keyword => Ok(Color::LightCyan),
            PythonToken::Literal => Ok(Color::LightMagenta),
            PythonToken::Decorator => Ok(Color::Cyan),
            PythonToken::String | PythonToken::SingleQuotedString => Ok(Color::LightGreen),
            PythonToken::Comment => Ok(Color::LightRed),
            PythonToken::Variable => Err(()),
        }
    }
}

#[derive(Debug)]
pub struct Python;

impl std::fmt::Display for Python {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        "Python".fmt(f)
    }
}

impl crate::syntax::Highlighter for Python {
    fn highlight<'s>(
        &self,
        s: &'s str,
    ) -> Box<dyn Iterator<Item = (Color, std::ops::Range<usize>)> + 's> {
        Box::new(
            PythonToken::lexer(s)
                .spanned()
                .filter_map(|(t, r)| t.ok().and_then(|t| Color::try_from(t).ok()).map(|c| (c, r))),
        )
    }
}
