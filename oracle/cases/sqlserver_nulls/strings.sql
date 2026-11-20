select upper(email) as e, len(name) as name_len, concat(name, '!') as excited, name + '!' as plus, left(email, 3) as head, trim(name) as trimmed, replace(email, '@', ' at ') as spelled from customers
