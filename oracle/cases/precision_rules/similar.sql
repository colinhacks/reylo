select first_name similar to 'A%' as s, first_name like 'A!%' escape '!' as e, first_name ~ '^A' as r, first_name !~* '^a' as n from member
