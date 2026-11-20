select starts_with(name, :prefix) as a, text_has_prefix(name, 'a') as b, text_has_suffix(nick, 'a') as c, text_contains(name, 'a') as d, text_like('a%', name) as e from people
