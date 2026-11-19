select id from vkey where slug = regexp_replace(:s, '\s', '', 'g')
