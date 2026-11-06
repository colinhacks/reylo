select max(name) as top, min(price) as low, max(at_ms) as latest, group_concat(code) as codes
from sized
