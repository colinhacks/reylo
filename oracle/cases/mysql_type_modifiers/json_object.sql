select json_object('name', name, 'code', code, 'body', body, 'raw', raw, 'price', price, 'at', at_ms, 'stamp', stamp, 'span', span_ms) as doc
from sized
