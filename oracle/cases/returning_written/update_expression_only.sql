update t set note = 'x' returning upper(note) as loud, note || label as joined, old.note || '' as was
