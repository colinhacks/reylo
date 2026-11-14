select p.id, p.title, case when :with_comments::bool then latest_comments(p, 10) end as comments from post p where p.id = :id::int8
