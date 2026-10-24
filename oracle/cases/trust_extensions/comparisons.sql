select age > 3 as gt, email = 'x' as eq, email <> 'y' as ne, age <= 2 as le, code = 'x' as vc, email like 'a%' as l, (age + 1) > 3 as nested from users
