SELECT count() AS n, max(age) AS oldest FROM users WHERE age > {than:UInt8}
