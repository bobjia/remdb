import io, re

MAP = {
    'bool': 'Bool', 'u8': 'U8', 'u16': 'U16', 'u32': 'U32', 'u64': 'U64',
    'i8': 'I8', 'i16': 'I16', 'i32': 'I32', 'i64': 'I64',
    'float32': 'Float32', 'float64': 'Float64', 'string': 'String',
}

for path in ['src/transaction.rs']:
    with io.open(path, 'r', encoding='utf-8') as f:
        content = f.read()
    def repl(m):
        field = m.group(1)
        val = m.group(2)
        if field not in MAP:
            return m.group(0)
        return f'crate::types::Value::{MAP[field]}({val})'
    new = re.sub(r'crate::types::Value \{ (\w+): ([^}]*?) \}', repl, content)
    with io.open(path, 'w', encoding='utf-8', newline='\n') as f:
        f.write(new)
    print(path, 'done')