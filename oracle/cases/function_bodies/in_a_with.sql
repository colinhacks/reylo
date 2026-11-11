with c as (select id, body from comments_for(:p::int8)) select id, body from c
