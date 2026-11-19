delete from vkey where email = coalesce(:e, '') returning id
