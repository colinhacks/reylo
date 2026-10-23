select bool_and(title <> '') as all_titled, bool_or(user_id = 1) as any_first, every(id > 0) as all_positive from posts
