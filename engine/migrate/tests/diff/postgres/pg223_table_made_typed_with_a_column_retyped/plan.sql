-- loses data: t.a changes from text to jsonb, which keeps the JSON's values, not its duplicate keys, key order or spacing
alter table t alter column a type jsonb using a::jsonb;
alter table t of ty;
