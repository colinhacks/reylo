SELECT quantile({level:Float64})(age) AS q, quantiles({low:Float64}, 0.9)(age) AS qs FROM users
