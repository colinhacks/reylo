select try_cast(n as tinyint) as tc, try_convert(tinyint, n) as tv, try_parse(s as int) as tp from f
