select /*+ SET_VAR(sort_buffer_size = 1048576) */ d.n from users x left join (select id, id + 1 as n from users) d on d.id = x.id + 1000
