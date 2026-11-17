select id from customers where id in (select value from openjson(:ids::rows(id int)))
