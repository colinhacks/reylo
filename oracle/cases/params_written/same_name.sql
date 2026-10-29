with o as (update other.users set name = :other_name where id = 1 returning id) update users set name = :name where id = 1
