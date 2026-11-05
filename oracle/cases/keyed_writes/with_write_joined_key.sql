with u as (insert into users (id, account_id) values (:id, :acct) returning *) select u.id, a.name from u join accounts a on a.id = u.account_id
