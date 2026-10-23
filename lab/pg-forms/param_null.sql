with input as (select :role::nullable(pgf_role) as role)
update pgf_users set email = :email, name = :name
from input
where pgf_users.role = coalesce(input.role, pgf_users.role)
returning id, email
