select k.id, o.note from k left join o on o.id = k.id where o.note is not null and k.id = 99 xor k.x = 1
