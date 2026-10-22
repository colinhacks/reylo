select users.id, merged.next from users left join merged on merged.user_id = users.id + 1000
