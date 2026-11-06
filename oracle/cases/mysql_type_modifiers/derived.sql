select d.name, d.legacy, d.body, d.raw, d.price, d.at_ms, d.span_ms
from (select name, legacy, body, raw, price, at_ms, span_ms from sized) d
