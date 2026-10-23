select a.*, b.name as later from users a left join users b on b.id > a.id
