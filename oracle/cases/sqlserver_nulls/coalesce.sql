select coalesce(name, email) as shown, isnull(credit, 0) as credit, coalesce(name, null) as still_null, nullif(email, '') as blank from customers
