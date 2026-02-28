extern crate std;

use std::{io::Read, iter::Peekable, str::FromStr, vec::Vec};

use crate::lexer::{LineToken, Token, TokenIterator};
use crate::repr::{
    Alignment, Brush, Edict, Entity, Point, Quake2SurfaceExtension, QuakeMap,
    Surface,
};
use crate::{TextParseError, TextParseResult};

const CELL_EXPECT: &str = "Expected cell value";

type TokenPeekable<R> = Peekable<TokenIterator<R>>;

trait Extract {
    fn extract(&mut self) -> TextParseResult<Option<LineToken>>;
}

impl<R> Extract for TokenPeekable<R>
where
    R: Read,
{
    fn extract(&mut self) -> Result<Option<LineToken>, TextParseError> {
        self.next()
            .transpose()
            .map_err(|e| e.into_inner().expect(CELL_EXPECT))
    }
}

const MIN_BRUSH_SURFACES: usize = 4;

/// Parses a Quake source map
///
/// Maps must be in the Quake 1 or 2 format (Quake 3 `brushDef`s/`patchDef`s are
/// not presently supported) but may have texture alignment in either "Valve220"
/// format or the "legacy" predecessor (i.e. without texture axes)
pub fn parse<R: Read>(reader: &mut R) -> TextParseResult<QuakeMap> {
    let mut entities: Vec<Entity> = Vec::new();
    let mut peekable_tokens = TokenIterator::new(reader).peekable();

    while peekable_tokens.peek().is_some() {
        let entity = parse_entity(&mut peekable_tokens)?;
        entities.push(entity);
    }

    Ok(QuakeMap { entities })
}

fn parse_entity<R: Read>(
    tokens: &mut TokenPeekable<R>,
) -> TextParseResult<Entity> {
    expect_token(&tokens.extract()?, Token::OpenCurly)?;

    let edict = parse_edict(tokens)?;
    let brushes = parse_brushes(tokens)?;

    expect_token(&tokens.extract()?, Token::CloseCurly)?;

    Ok(Entity { edict, brushes })
}

fn parse_edict<R: Read>(
    tokens: &mut TokenPeekable<R>,
) -> TextParseResult<Edict> {
    let mut edict = Edict::new();

    while let Some(tok_res) = tokens.peek() {
        if tok_res
            .as_ref()
            .map_err(|e| e.take().expect(CELL_EXPECT))?
            .is_quoted()
        {
            let key = tokens.extract()?.unwrap().into_bare_cstring();
            let maybe_value = tokens.extract()?;
            expect_quoted(&maybe_value)?;
            let value = maybe_value.unwrap().into_bare_cstring();
            edict.push((key, value));
        } else {
            break;
        }
    }

    Ok(edict)
}

fn parse_brushes<R: Read>(
    tokens: &mut TokenPeekable<R>,
) -> TextParseResult<Vec<Brush>> {
    let mut brushes = Vec::new();

    while let Some(tok_res) = tokens.peek() {
        if tok_res
            .as_ref()
            .map_err(|e| e.take().expect(CELL_EXPECT))?
            .token
            == Token::OpenCurly
        {
            brushes.push(parse_brush(tokens)?);
        } else {
            break;
        }
    }

    Ok(brushes)
}

fn parse_brush<R: Read>(
    tokens: &mut TokenPeekable<R>,
) -> TextParseResult<Brush> {
    let mut surfaces = Vec::with_capacity(MIN_BRUSH_SURFACES);
    expect_token(&tokens.extract()?, Token::OpenCurly)?;

    while let Some(tok_res) = tokens.peek() {
        if tok_res
            .as_ref()
            .map_err(|e| e.take().expect(CELL_EXPECT))?
            .token
            == Token::OpenParen
        {
            surfaces.push(parse_surface(tokens)?);
        } else {
            break;
        }
    }

    expect_token_or(&tokens.extract()?, Token::CloseCurly, b"(")?;
    Ok(surfaces)
}

fn parse_surface<R: Read>(
    tokens: &mut TokenPeekable<R>,
) -> TextParseResult<Surface> {
    let pt1 = parse_point(tokens)?;
    let pt2 = parse_point(tokens)?;
    let pt3 = parse_point(tokens)?;

    let half_space = [pt1, pt2, pt3];

    let texture_token = tokens.extract()?.ok_or_else(TextParseError::eof)?;

    let texture = texture_token.into_bare_cstring();

    let alignment = if let Some(tok_res) = tokens.peek() {
        if tok_res
            .as_ref()
            .map_err(|e| e.take().expect(CELL_EXPECT))?
            .token
            == Token::OpenSquare
        {
            parse_valve_alignment(tokens)?
        } else {
            parse_legacy_alignment(tokens)?
        }
    } else {
        return Err(TextParseError::eof());
    };

    let q2ext = if let Some(tok_res) = tokens.peek() {
        if matches!(
            tok_res
                .as_ref()
                .map_err(|e| e.take().expect(CELL_EXPECT))?
                .token,
            Token::BareString(_),
        ) {
            parse_q2_ext(tokens)?
        } else {
            Default::default()
        }
    } else {
        return Err(TextParseError::eof());
    };

    Ok(Surface {
        half_space,
        texture,
        alignment,
        q2ext,
    })
}

fn parse_point<R: Read>(
    tokens: &mut TokenPeekable<R>,
) -> TextParseResult<Point> {
    expect_token(&tokens.extract()?, Token::OpenParen)?;
    let x = expect_float(&tokens.extract()?)?;
    let y = expect_float(&tokens.extract()?)?;
    let z = expect_float(&tokens.extract()?)?;
    expect_token(&tokens.extract()?, Token::CloseParen)?;

    Ok([x, y, z])
}

fn parse_legacy_alignment<R: Read>(
    tokens: &mut TokenPeekable<R>,
) -> TextParseResult<Alignment> {
    let offset_x = expect_float(&tokens.extract()?)?;
    let offset_y = expect_float(&tokens.extract()?)?;
    let rotation = expect_float(&tokens.extract()?)?;
    let scale_x = expect_float(&tokens.extract()?)?;
    let scale_y = expect_float(&tokens.extract()?)?;

    Ok(Alignment {
        offset: [offset_x, offset_y],
        rotation,
        scale: [scale_x, scale_y],
        axes: None,
    })
}

fn parse_q2_ext<R: Read>(
    tokens: &mut TokenPeekable<R>,
) -> TextParseResult<Quake2SurfaceExtension> {
    let content_flags = expect_int(&tokens.extract()?)?;
    let surface_flags = expect_int(&tokens.extract()?)?;
    let surface_value = expect_float(&tokens.extract()?)?;

    Ok(Quake2SurfaceExtension {
        content_flags,
        surface_flags,
        surface_value,
    })
}

fn parse_valve_alignment<R: Read>(
    tokens: &mut TokenPeekable<R>,
) -> TextParseResult<Alignment> {
    expect_token(&tokens.extract()?, Token::OpenSquare)?;
    let u_x = expect_float(&tokens.extract()?)?;
    let u_y = expect_float(&tokens.extract()?)?;
    let u_z = expect_float(&tokens.extract()?)?;
    let offset_x = expect_float(&tokens.extract()?)?;
    expect_token(&tokens.extract()?, Token::CloseSquare)?;

    expect_token(&tokens.extract()?, Token::OpenSquare)?;
    let v_x = expect_float(&tokens.extract()?)?;
    let v_y = expect_float(&tokens.extract()?)?;
    let v_z = expect_float(&tokens.extract()?)?;
    let offset_y = expect_float(&tokens.extract()?)?;
    expect_token(&tokens.extract()?, Token::CloseSquare)?;

    let rotation = expect_float(&tokens.extract()?)?;
    let scale_x = expect_float(&tokens.extract()?)?;
    let scale_y = expect_float(&tokens.extract()?)?;

    Ok(Alignment {
        offset: [offset_x, offset_y],
        rotation,
        scale: [scale_x, scale_y],
        axes: Some([[u_x, u_y, u_z], [v_x, v_y, v_z]]),
    })
}

fn expect_token(
    line_token: &Option<LineToken>,
    token: Token,
) -> TextParseResult<()> {
    match line_token.as_ref() {
        Some(payload) if payload.token == token => Ok(()),
        Some(payload) => Err(TextParseError::from_parser(
            format!("Expected `{}`, got `{}`", token, payload.token),
            payload.line_number,
        )),
        _ => Err(TextParseError::eof()),
    }
}

fn expect_token_or(
    line_token: &Option<LineToken>,
    token: Token,
    rest: &[u8],
) -> TextParseResult<()> {
    match line_token.as_ref() {
        Some(payload) if payload.token == token => Ok(()),
        Some(payload) => {
            let rest_str = rest
                .iter()
                .copied()
                .map(|b| format!("`{}`", char::from(b)))
                .collect::<Vec<_>>()[..]
                .join(", ");

            Err(TextParseError::from_parser(
                format!(
                    "Expected {} or `{}`, got `{}`",
                    rest_str, token, payload.token
                ),
                payload.line_number,
            ))
        }
        _ => Err(TextParseError::eof()),
    }
}

fn expect_quoted(token: &Option<LineToken>) -> TextParseResult<()> {
    match token.as_ref() {
        Some(payload) if payload.is_quoted() => Ok(()),
        Some(payload) => Err(TextParseError::from_parser(
            format!("Expected quoted, got `{}`", payload.token),
            payload.line_number,
        )),
        _ => Err(TextParseError::eof()),
    }
}

fn expect_float(token: &Option<LineToken>) -> TextParseResult<f64> {
    match token.as_ref() {
        Some(payload) => match f64::from_str(payload.token.as_number_text()) {
            Ok(num) => Ok(num),
            Err(_) => Err(TextParseError::from_parser(
                format!("Expected number, got `{}`", payload.token),
                payload.line_number,
            )),
        },
        None => Err(TextParseError::eof()),
    }
}

fn expect_int(token: &Option<LineToken>) -> TextParseResult<i32> {
    match token.as_ref() {
        Some(payload) => match i32::from_str(payload.token.as_number_text()) {
            Ok(num) => Ok(num),
            Err(_) => Err(TextParseError::from_parser(
                format!("Expected integer, got `{}`", payload.token),
                payload.line_number,
            )),
        },
        None => Err(TextParseError::eof()),
    }
}
