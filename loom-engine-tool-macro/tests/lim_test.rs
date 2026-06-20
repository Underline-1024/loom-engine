#[cfg(test)]
mod tests {
    use loom_engine::lim::Lim;

    #[test]
    fn test_set_bounds_clamp_to_exclusive_min_does_not_create_invalid_state() {
        // 初始状态：区间 (1, 10)，value = 1.5（在范围内，因为 >1）
        let mut lim = Lim::new(1.5, 1.0, 10.0, false, false).unwrap();
        
        // 尝试把下界改成 2.0，不包含，启用钳制
        // 当前 value = 1.5 < 2.0，应该无法钳制到排他边界
        let result = lim.set_bounds(2.0, 10.0, false, false, true);
        
        // 期望：报错，不应该成功钳制
        assert!(result.is_err());
        
        // 如果钳制成功了，value 会被改成 2.0，但区间是 (2, 10)，2.0 不应该被包含
        // 这里验证 value 没有变成非法值
        if let Ok(()) = result {
            assert!(!lim.contains(&2.0), "Value 2.0 should NOT be in exclusive range (2, 10)");
            assert!(lim.value() > lim.min() || lim.include_min(), 
                "Value should not equal exclusive min");
        }
    }

    #[test]
    fn test_set_bounds_clamp_to_exclusive_max_does_not_create_invalid_state() {
        // 初始状态：区间 (1, 10)，value = 9.5（在范围内，因为 <10）
        let mut lim = Lim::new(9.5, 1.0, 10.0, false, false).unwrap();
        
        // 尝试把上界改成 9.0，不包含，启用钳制
        // 当前 value = 9.5 > 9.0，应该无法钳制到排他边界
        let result = lim.set_bounds(1.0, 9.0, false, false, true);
        
        // 期望：报错
        assert!(result.is_err());
        
        // 验证没有产生非法状态
        if let Ok(()) = result {
            assert!(!lim.contains(&9.0), "Value 9.0 should NOT be in exclusive range (1, 9)");
            assert!(lim.value() < lim.max() || lim.include_max(),
                "Value should not equal exclusive max");
        }
    }

    #[test]
    fn test_set_bounds_clamp_to_exclusive_min_with_value_exactly_equal_is_ambiguous() {
        // 初始状态：区间 (1, 10)，value = 5
        let mut lim = Lim::new(5.0, 1.0, 10.0, false, false).unwrap();
        
        // 尝试把下界改成 5，不包含，启用钳制
        // value == min && !include_min → 歧义情况，应该报错
        let result = lim.set_bounds(5.0, 10.0, false, false, true);
        
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Ambiguous") || err_msg.contains("exclusive min"));
    }

    #[test]
    fn test_set_bounds_clamp_to_exclusive_max_with_value_exactly_equal_is_ambiguous() {
        // 初始状态：区间 (1, 10)，value = 5
        let mut lim = Lim::new(5.0, 1.0, 10.0, false, false).unwrap();
        
        // 尝试把上界改成 5，不包含，启用钳制
        let result = lim.set_bounds(1.0, 5.0, false, false, true);
        
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Ambiguous") || err_msg.contains("exclusive max"));
    }

    #[test]
    fn test_set_min_rejects_clamp_to_exclusive_boundary() {
        // 这个测试验证 set_min 的现有行为（作为对照）
        let mut lim = Lim::new(1.5, 1.0, 10.0, false, false).unwrap();
        
        // set_min 应该拒绝钳制到排他边界
        let result = lim.set_min(2.0, false, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_bounds_clamp_to_inclusive_boundary_works() {
        // 验证包含边界时可以正常钳制（作为正例对照）
        let mut lim = Lim::new(0.5, 0.0, 10.0, true, true).unwrap();
        
        // 下界改成 1.0，包含，当前 value=0.5 < 1.0 → 应该钳制到 1.0
        let result = lim.set_bounds(1.0, 10.0, true, true, true);
        assert!(result.is_ok());
        assert_eq!(*lim.value(), 1.0);
    }
}