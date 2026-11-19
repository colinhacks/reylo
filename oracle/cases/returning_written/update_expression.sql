update t set note = coalesce(note, label, '') returning note
