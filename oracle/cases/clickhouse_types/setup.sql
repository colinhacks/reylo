CREATE TABLE every (
  id UInt64,
  u8 UInt8, u16 UInt16, u32 UInt32, u128 UInt128, u256 UInt256,
  i8 Int8, i16 Int16, i32 Int32, i64 Int64, i128 Int128, i256 Int256,
  f32 Float32, f64 Float64,
  d32 Decimal32(4), d64 Decimal(18, 6), d128 Decimal(38, 10), d256 Decimal(76, 20),
  flag Bool,
  s String, fixed FixedString(4), packed LowCardinality(String),
  uid UUID, ip4 IPv4, ip6 IPv6,
  day Date, day32 Date32, at DateTime('UTC'), at_zone DateTime('Asia/Tokyo'), at3 DateTime64(3, 'UTC'), at9 DateTime64(9, 'UTC'),
  role Enum8('admin' = 1, 'member' = 2), big_role Enum16('a' = -300, 'b' = 300),
  tags Array(String), holes Array(Nullable(Int32)), deep Array(Array(UInt8)),
  pair Tuple(UInt8, String), named Tuple(x Float64, label Nullable(String)),
  counts Map(String, UInt32),
  maybe Nullable(String), maybe_n Nullable(Int64), maybe_packed LowCardinality(Nullable(String)),
  spot Point
) ENGINE = MergeTree ORDER BY id;
INSERT INTO every VALUES
  (1, 255, 65535, 4294967295, 340282366920938463463374607431768211455, 115792089237316195423570985008687907853269984665640564039457584007913129639935,
   -128, -32768, -2147483648, -9223372036854775808, -170141183460469231731687303715884105728, -57896044618658097711785492504343953926634992332820282019728792003956564819968,
   -1.5, 1e308, 99999.9999, -999999999999.999999, 1234567890123456789012345678.0123456789, 0.00000000000000000001,
   true, 'héllo 😀', 'ab', 'pro',
   '61f0c404-5cb3-11e7-907b-a6006ad3dba0', '255.255.255.255', '2001:db8::1',
   '2149-06-06', '2299-12-31', '2106-02-07 06:28:15', '2026-01-01 21:00:00', '2026-01-02 03:04:05.678', '2026-01-02 03:04:05.123456789',
   'admin', 'b', ['a', ''], [1, NULL], [[1, 2], []], (1, 'x'), (1.5, NULL), map('k', 1, 'k', 2), 'text', -1, 'p', (1.5, -2.5)),
  (2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, nan, -inf, 0, 0, 0, 0, false, '', '', '', '00000000-0000-0000-0000-000000000000', '0.0.0.0', '::',
   '1970-01-01', '1900-01-01', '1970-01-01 00:00:00', '1970-01-01 09:00:00', '1900-01-01 00:00:00', '1969-12-31 23:59:59.5',
   'member', 'a', [], [], [], (0, ''), (0, 'l'), map(), NULL, NULL, NULL, (0, 0));
CREATE TABLE baskets (id UInt64, items Nested(sku String, quantity UInt32), `odd name` UInt8, `select` String) ENGINE = MergeTree ORDER BY id;
INSERT INTO baskets VALUES (1, ['a', 'b'], [1, 2], 7, 'x'), (2, [], [], 0, '')
