select a.name as earlier, b.* from users a left join users b on b.id > a.id
