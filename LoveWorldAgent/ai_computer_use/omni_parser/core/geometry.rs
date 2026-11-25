#[derive(Debug, Clone, Copy, PartialEq)] // 归一化矩形框
pub struct BBox { pub x1: f32, pub y1: f32, pub x2: f32, pub y2: f32 } // 左上右下
impl BBox {
    pub fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self { Self { x1, y1, x2, y2 } } // 构造
    pub fn area(&self) -> f32 { (self.x2 - self.x1) * (self.y2 - self.y1) } // 面积
    pub fn intersection_area(&self, other: &BBox) -> f32 { // 交集面积
        let ix = (self.x2.min(other.x2) - self.x1.max(other.x1)).max(0.0); // 横向交集
        let iy = (self.y2.min(other.y2) - self.y1.max(other.y1)).max(0.0); // 纵向交集
        ix * iy // 返回交集
    }
    pub fn iou(&self, other: &BBox) -> f32 { // 标准交并比
        let inter = self.intersection_area(other); // 交集
        let a = self.area(); // 自身面积
        let b = other.area(); // 对方面积
        inter / (a + b - inter + 1e-6) // 交并比
    }
    pub fn overlaps(&self, other: &BBox) -> bool { // 覆盖率超 80% 视为重叠
        let inter = self.intersection_area(other); // 交集
        let a = self.area(); // 自身面积
        if a <= 0.0 { return false; } // 空框不重叠
        inter / a > 0.80 // 阈值判定
    }
}