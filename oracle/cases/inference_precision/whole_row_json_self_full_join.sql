select to_jsonb(a) as earlier, to_jsonb(b) as later from users a full join users b on b.id = a.id + 1
