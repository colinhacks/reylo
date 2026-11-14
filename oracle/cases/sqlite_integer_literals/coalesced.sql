select coalesce(b, 9223372036854775808) as past, coalesce(b, -9223372036854775808) as min, coalesce(b, 1) as one from t
