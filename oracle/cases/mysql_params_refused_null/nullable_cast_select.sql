insert into accounts (id, email) select :id, :email::nullable(text)
