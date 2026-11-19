SELECT count() AS n FROM users QUALIFY row_number() OVER () > 1
