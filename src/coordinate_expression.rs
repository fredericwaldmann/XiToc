//! Bounded arithmetic grammar for scientific plots.
#[derive(Debug)]
pub enum Expr {
    Value(f64),
    X,
    Y,
    Neg(Box<Self>),
    Binary(u8, Box<Self>, Box<Self>),
    Call(fn(f64) -> f64, Box<Self>),
}

impl Expr {
    pub fn evaluate(&self, x: f64, y: f64) -> f64 {
        match self {
            Self::Value(value) => *value,
            Self::X => x,
            Self::Y => y,
            Self::Neg(value) => -value.evaluate(x, y),
            Self::Binary(op, left, right) => {
                let a = left.evaluate(x, y);
                let b = right.evaluate(x, y);
                match op {
                    b'+' => a + b,
                    b'-' => a - b,
                    b'*' => a * b,
                    b'/' => a / b,
                    _ => a.powf(b),
                }
            }
            Self::Call(function, argument) => function(argument.evaluate(x, y)),
        }
    }
}

pub fn parse(source: &str, surface: bool) -> Result<Expr, String> {
    if source.len() > 512 {
        return Err("An expression can contain up to 512 UTF-8 bytes.".into());
    }
    let input = if let Some((lhs, rhs)) = source.split_once('=') {
        let lhs = lhs.trim();
        let signature = lhs.split_once('(').is_some_and(|(name, args)| {
            !name.trim().is_empty()
                && name.trim().bytes().all(|c| c.is_ascii_alphabetic())
                && args.strip_suffix(')').is_some_and(|args| {
                    let args: Vec<_> = args.split(',').map(str::trim).collect();
                    if surface {
                        args == ["x", "y"]
                    } else {
                        args == ["x"]
                    }
                })
        });
        if lhs != if surface { "z" } else { "y" } && !signature {
            return Err("Use an expression or a matching function assignment.".into());
        }
        rhs
    } else {
        source
    };
    let mut parser = Parser {
        rest: input,
        remaining: 128,
        surface,
    };
    let result = parser.expression(0)?;
    if !parser.rest.trim().is_empty() {
        return Err(format!("Unexpected input near “{}”.", parser.rest.trim()));
    }
    Ok(result)
}

struct Parser<'a> {
    rest: &'a str,
    remaining: usize,
    surface: bool,
}

impl Parser<'_> {
    fn next(&mut self) -> Option<u8> {
        self.rest = self.rest.trim_start();
        self.rest.as_bytes().first().copied()
    }

    fn expression(&mut self, minimum: u8) -> Result<Expr, String> {
        let mut left = self.atom()?;
        while let Some(op) = self.next() {
            let (low, high) = match op {
                b'+' | b'-' => (1, 2),
                b'*' | b'/' => (3, 4),
                b'^' => (6, 5),
                _ => break,
            };
            if low < minimum {
                break;
            }
            self.rest = &self.rest[1..];
            left = Expr::Binary(op, Box::new(left), Box::new(self.expression(high)?));
        }
        Ok(left)
    }

    fn group(&mut self) -> Result<Expr, String> {
        if self.next() != Some(b'(') {
            return Err("Expected an opening parenthesis.".into());
        }
        self.rest = &self.rest[1..];
        let value = self.expression(0)?;
        if self.next() != Some(b')') {
            return Err("Expected a closing parenthesis.".into());
        }
        self.rest = &self.rest[1..];
        Ok(value)
    }

    fn atom(&mut self) -> Result<Expr, String> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or("Expression is too complex.")?;
        match self.next() {
            Some(b'(') => self.group(),
            Some(sign @ (b'-' | b'+')) => {
                self.rest = &self.rest[1..];
                let value = self.expression(5)?;
                Ok(if sign == b'-' {
                    Expr::Neg(Box::new(value))
                } else {
                    value
                })
            }
            Some(b'0'..=b'9' | b'.') => {
                let mut length = 0;
                let mut previous = 0;
                for byte in self.rest.bytes() {
                    if !(byte.is_ascii_digit()
                        || matches!(byte, b'.' | b'e' | b'E')
                        || matches!(byte, b'+' | b'-') && matches!(previous, b'e' | b'E'))
                    {
                        break;
                    }
                    length += 1;
                    previous = byte;
                }
                let value = self.rest[..length]
                    .parse::<f64>()
                    .ok()
                    .filter(|n| n.is_finite())
                    .ok_or("Invalid finite number.")?;
                self.rest = &self.rest[length..];
                Ok(Expr::Value(value))
            }
            Some(byte) if byte.is_ascii_alphabetic() || self.rest.starts_with('π') => {
                let length = if self.rest.starts_with('π') {
                    'π'.len_utf8()
                } else {
                    self.rest
                        .bytes()
                        .take_while(u8::is_ascii_alphanumeric)
                        .count()
                };
                let (name, rest) = self.rest.split_at(length);
                self.rest = rest;
                let function: fn(f64) -> f64 = match name {
                    "x" => return Ok(Expr::X),
                    "y" if self.surface => return Ok(Expr::Y),
                    "pi" | "π" => return Ok(Expr::Value(std::f64::consts::PI)),
                    "e" => return Ok(Expr::Value(std::f64::consts::E)),
                    "sin" => f64::sin,
                    "cos" => f64::cos,
                    "tan" => f64::tan,
                    "sinh" => f64::sinh,
                    "cosh" => f64::cosh,
                    "tanh" => f64::tanh,
                    "asin" => f64::asin,
                    "acos" => f64::acos,
                    "atan" => f64::atan,
                    "sqrt" => f64::sqrt,
                    "abs" => f64::abs,
                    "exp" => f64::exp,
                    "ln" | "log" => f64::ln,
                    "log10" => f64::log10,
                    "floor" => f64::floor,
                    "ceil" => f64::ceil,
                    _ => return Err(format!("Unknown variable or function “{name}”.")),
                };
                Ok(Expr::Call(function, Box::new(self.group()?)))
            }
            _ => Err("Expected a number, variable, or mathematical function.".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn evaluates_bounded_arithmetic_with_standard_precedence() {
        for (source, expected) in [
            ("y=x", 2.),
            ("f(x)=x^2", 4.),
            ("-2^2+2^3^2", 508.),
            ("cos(pi)+sin(pi/2)", 0.),
            ("exp(-x)", (-2_f64).exp()),
            ("sqrt(x)+1e-2", 2_f64.sqrt() + 0.01),
            ("log10(100)", 2.),
        ] {
            assert!(
                (parse(source, false).unwrap().evaluate(2., 3.) - expected).abs() < 1e-9,
                "{source}"
            );
        }
        assert_eq!(parse("z=x^2+y^2", true).unwrap().evaluate(2., 3.), 13.);
        assert!(parse("sqrt(-1)", false).unwrap().evaluate(0., 0.).is_nan());
        assert!(parse("1/x", false).unwrap().evaluate(0., 0.).is_infinite());
        for invalid in [
            "",
            "system(x)",
            "y+y",
            "sin x",
            "2x",
            "1e999",
            "x=2",
            "(1",
            "x^^3",
        ] {
            assert!(parse(invalid, false).is_err(), "{invalid}");
        }
        assert!(parse(&"-".repeat(200), false).is_err());
    }
}
