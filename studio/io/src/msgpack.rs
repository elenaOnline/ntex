//! The little of MessagePack that nvim's RPC needs (the Terminal page's
//! capture talks to `nvim --embed` as a UI): values, read from a stream and
//! written to bytes.
use std::io::Read;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    F64(f64),
    Str(String),
    Bin(Vec<u8>),
    Array(Vec<Value>),
    Map(Vec<(Value, Value)>),
    /// nvim's Buffer / Window / Tabpage handles.
    Ext(i8, Vec<u8>),
}

impl Value {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }
    /// A map entry by string key.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(m) => m.iter().find(|(k, _)| k.as_str() == Some(key)).map(|(_, v)| v),
            _ => None,
        }
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::Str(s.to_string())
    }
}
impl From<i64> for Value {
    fn from(i: i64) -> Value {
        Value::Int(i)
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bool(b)
    }
}

fn byte(r: &mut impl Read) -> std::io::Result<u8> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b)?;
    Ok(b[0])
}

fn bytes(r: &mut impl Read, n: usize) -> std::io::Result<Vec<u8>> {
    let mut v = vec![0u8; n];
    r.read_exact(&mut v)?;
    Ok(v)
}

fn be(r: &mut impl Read, n: usize) -> std::io::Result<u64> {
    let b = bytes(r, n)?;
    Ok(b.iter().fold(0u64, |a, x| a << 8 | *x as u64))
}

/// Read one value (blocks until it is whole).
pub fn read(r: &mut impl Read) -> std::io::Result<Value> {
    let t = byte(r)?;
    Ok(match t {
        0x00..=0x7f => Value::Int(t as i64),
        0x80..=0x8f => map(r, (t & 0x0f) as usize)?,
        0x90..=0x9f => array(r, (t & 0x0f) as usize)?,
        0xa0..=0xbf => string(r, (t & 0x1f) as usize)?,
        0xc0 => Value::Nil,
        0xc2 => Value::Bool(false),
        0xc3 => Value::Bool(true),
        0xc4 => {
            let n = be(r, 1)? as usize;
            Value::Bin(bytes(r, n)?)
        }
        0xc5 => {
            let n = be(r, 2)? as usize;
            Value::Bin(bytes(r, n)?)
        }
        0xc6 => {
            let n = be(r, 4)? as usize;
            Value::Bin(bytes(r, n)?)
        }
        0xc7..=0xc9 => {
            let n = be(r, 1 << (t - 0xc7))? as usize;
            let ty = byte(r)? as i8;
            Value::Ext(ty, bytes(r, n)?)
        }
        0xca => Value::F64(f32::from_bits(be(r, 4)? as u32) as f64),
        0xcb => Value::F64(f64::from_bits(be(r, 8)?)),
        0xcc => Value::Int(be(r, 1)? as i64),
        0xcd => Value::Int(be(r, 2)? as i64),
        0xce => Value::Int(be(r, 4)? as i64),
        0xcf => Value::Int(be(r, 8)? as i64),
        0xd0 => Value::Int(be(r, 1)? as u8 as i8 as i64),
        0xd1 => Value::Int(be(r, 2)? as u16 as i16 as i64),
        0xd2 => Value::Int(be(r, 4)? as u32 as i32 as i64),
        0xd3 => Value::Int(be(r, 8)? as i64),
        0xd4..=0xd8 => {
            let n = 1usize << (t - 0xd4);
            let ty = byte(r)? as i8;
            Value::Ext(ty, bytes(r, n)?)
        }
        0xd9 => {
            let n = be(r, 1)? as usize;
            string(r, n)?
        }
        0xda => {
            let n = be(r, 2)? as usize;
            string(r, n)?
        }
        0xdb => {
            let n = be(r, 4)? as usize;
            string(r, n)?
        }
        0xdc => {
            let n = be(r, 2)? as usize;
            array(r, n)?
        }
        0xdd => {
            let n = be(r, 4)? as usize;
            array(r, n)?
        }
        0xde => {
            let n = be(r, 2)? as usize;
            map(r, n)?
        }
        0xdf => {
            let n = be(r, 4)? as usize;
            map(r, n)?
        }
        0xe0..=0xff => Value::Int(t as i8 as i64),
        0xc1 => return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "msgpack: 0xc1")),
    })
}

fn string(r: &mut impl Read, n: usize) -> std::io::Result<Value> {
    Ok(Value::Str(String::from_utf8_lossy(&bytes(r, n)?).into_owned()))
}

fn array(r: &mut impl Read, n: usize) -> std::io::Result<Value> {
    let mut v = Vec::with_capacity(n.min(4096));
    for _ in 0..n {
        v.push(read(r)?);
    }
    Ok(Value::Array(v))
}

fn map(r: &mut impl Read, n: usize) -> std::io::Result<Value> {
    let mut v = Vec::with_capacity(n.min(4096));
    for _ in 0..n {
        let k = read(r)?;
        v.push((k, read(r)?));
    }
    Ok(Value::Map(v))
}

/// Write one value.
pub fn write(v: &Value, out: &mut Vec<u8>) {
    let len = |out: &mut Vec<u8>, n: usize, fix: u8, fix_max: usize, b16: u8, b32: u8| {
        if n <= fix_max {
            out.push(fix | n as u8);
        } else if n <= 0xffff {
            out.push(b16);
            out.extend_from_slice(&(n as u16).to_be_bytes());
        } else {
            out.push(b32);
            out.extend_from_slice(&(n as u32).to_be_bytes());
        }
    };
    match v {
        Value::Nil => out.push(0xc0),
        Value::Bool(b) => out.push(if *b { 0xc3 } else { 0xc2 }),
        Value::Int(i) => {
            if (0..=0x7f).contains(i) {
                out.push(*i as u8);
            } else if (-32..0).contains(i) {
                out.push(*i as i8 as u8);
            } else {
                out.push(0xd3);
                out.extend_from_slice(&i.to_be_bytes());
            }
        }
        Value::F64(f) => {
            out.push(0xcb);
            out.extend_from_slice(&f.to_bits().to_be_bytes());
        }
        Value::Str(s) => {
            if s.len() <= 31 {
                out.push(0xa0 | s.len() as u8);
            } else if s.len() <= 0xff {
                out.push(0xd9);
                out.push(s.len() as u8);
            } else if s.len() <= 0xffff {
                out.push(0xda);
                out.extend_from_slice(&(s.len() as u16).to_be_bytes());
            } else {
                out.push(0xdb);
                out.extend_from_slice(&(s.len() as u32).to_be_bytes());
            }
            out.extend_from_slice(s.as_bytes());
        }
        Value::Bin(b) => {
            out.push(0xc6);
            out.extend_from_slice(&(b.len() as u32).to_be_bytes());
            out.extend_from_slice(b);
        }
        Value::Array(a) => {
            len(out, a.len(), 0x90, 15, 0xdc, 0xdd);
            for x in a {
                write(x, out);
            }
        }
        Value::Map(m) => {
            len(out, m.len(), 0x80, 15, 0xde, 0xdf);
            for (k, x) in m {
                write(k, out);
                write(x, out);
            }
        }
        Value::Ext(ty, data) => {
            out.push(0xc7);
            out.push(data.len() as u8);
            out.push(*ty as u8);
            out.extend_from_slice(data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let v = Value::Array(vec![
            Value::Int(0),
            Value::Int(-5),
            Value::Int(300),
            Value::Int(-70000),
            Value::Str("grid_line".into()),
            Value::Str("x".repeat(300)),
            Value::Map(vec![(Value::from("rgb"), Value::Bool(true))]),
            Value::Nil,
            Value::F64(1.5),
            Value::Ext(1, vec![3]),
        ]);
        let mut b = Vec::new();
        write(&v, &mut b);
        assert_eq!(read(&mut &b[..]).unwrap(), v);
    }

    #[test]
    fn reads_small_forms() {
        // fixmap {"a": uint16 513}, int8 -100, fixext1
        let b = [0x81, 0xa1, b'a', 0xcd, 0x02, 0x01, 0xd0, 0x9c, 0xd4, 0x01, 0x07];
        let mut r = &b[..];
        assert_eq!(read(&mut r).unwrap().get("a").and_then(|v| v.as_i64()), Some(513));
        assert_eq!(read(&mut r).unwrap(), Value::Int(-100));
        assert_eq!(read(&mut r).unwrap(), Value::Ext(1, vec![7]));
    }
}
