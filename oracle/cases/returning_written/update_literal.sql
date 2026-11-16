update t set note = 'x', at = now() where id = 1 returning note, at, n
