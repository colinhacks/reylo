select a.id, to_jsonb(a) as earlier, to_jsonb(b) as later from users a left join users b on b.id > a.id
