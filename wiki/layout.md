## 窗口布局管理数据结构

### 引言

在一个窗口管理器中，布局系统扮演着核心角色。为了高效管理窗口的空间排布，本项目采用了一种结构清晰、修改高效的二叉树（Binary Tree） 结构作为窗口布局的基础数据模型。该树结构基于 `SlotMap` 构建，结合唯一键值索引（Key-based access），将常规操作如插入、删除、定位的时间复杂度优化至常数级别 `O(1)`。

为了管理和组织当前活动窗口的空间结构，在 `Workspace` 结构体中维护了两个核心字段：

```rust
#[derive(Debug)]
pub struct Workspace {
    ...
    pub layout: LayoutScheme,
    pub layout_tree: Option<TiledTree>,
}
```

- `layout`：用于指示当前使用的窗口排列方案。
- `layout_tree`：存储当前工作区内窗口的具体排布信息，其核心数据结构即为 `TiledTree`。

### 数据结构设计

#### 布局方案枚举

为布局方案定义枚举类型，详细算法见 自动布局算法。

```rust
#[derive(Debug, Clone)]
pub enum LayoutScheme {
    Default,
    BinaryTree,
}
```

#### 节点信息设计

```rust
#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub enum NodeData {
    Leaf { window: Window },
    Split {
        direction: Direction,
        rec: Rectangle<i32, Logical>,
        offset: (i32, i32),
        left: NodeId,
        right: NodeId,
    }
}
```

- **内部节点（父节点）**：代表一个区域被划分的逻辑，存储以下信息：
  
  - 分裂方向：水平（Horizontal）或垂直（Vertical）
  - 窗口的起点位置与总大小。
  - 子节点的索引（左子节点与右子节点）
  - offset：内部分裂的偏差值（用于手动更新窗口大小）

- **叶子节点**：表示一个具体窗口的存在，只存储窗口 ID（或 Surface ID），不再包含其他布局信息。

> 说明：每次添加新窗口时，目标叶子节点会被替换为一个新的父节点，并且规定左子节点处于布局的上方/左侧，其两个子节点分别为原窗口和新窗口的 ID。

#### 窗口 UserData

为 `Window` 类型新增一个 `tarit`，用于存储/修改窗口的布局信息（大小与位置）。

```rust
pub trait LayoutHandle {
    fn set_rec(&self, new_rec: Rectangle<i32, Logical>);
    fn get_rec(&self) -> Option<Rectangle<i32, Logical>>;
}

impl LayoutHandle for Window {
    fn set_rec(&self, new_rec: Rectangle<i32, Logical>) {
        if let Some(e) = self
            .user_data()
            .get::<RefCell<WindowExtElements>>() 
        {
            e.borrow_mut().rec = new_rec;
        }
    }

    fn get_rec(&self) -> Option<Rectangle<i32, Logical>> {
        self.
            user_data()
            .get::<RefCell<WindowExtElements>>()
            .and_then(|e| Some(e.borrow().rec.clone()))
    }
}
```

#### SlotMap

为提升树结构的动态操作性能，本项目引入了 [Rust 的 SlotMap](https://docs.rs/slotmap/) 作为节点存储的底层容器。相比传统引用或 `Box` 指针，`SlotMap` 具有以下优势：

- **快速访问**：所有节点通过唯一的 Key 标识，可在 O(1) 时间内访问。
- **插入与删除开销小**：不影响其他节点位置，避免指针更新或数据重排。
- **避免悬垂指针问题**：因为节点通过 key 而非裸指针引用，内存安全性更高。

每个节点在 `SlotMap` 中都会分配一个唯一的 `NodeId`，父节点只需保存左右子节点的 `NodeId`，大大简化了树的管理和操作逻辑。

以下是 `TiledTree` 的基础定义：

```rust
use slotmap::{new_key_type, SlotMap};

new_key_type! {
    pub struct NodeId;
}

#[derive(Debug)]
pub struct TiledTree {
    nodes: SlotMap<NodeId, NodeData>,
    root: Option<NodeId>,
}
```

在这个结构中：

- `nodes`：维护了整个布局树中所有节点的数据。
- `root`：指向当前布局树的根节点，如果树为空，则为 `None`。

以下是创建树与一些必要的工具函数：

```rust
impl TiledTree {
    pub fn new(window: Window) -> Self {
        let mut nodes = SlotMap::with_key();
        let root = Some(nodes.insert(NodeData::Leaf { window }));
        Self { 
            nodes,
            root
       }
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub fn find_node(&self, window: &Window) -> Option<NodeId> {
        self.nodes.iter()
            .find_map(|(id, data)| match data {
                NodeData::Leaf { window: w } if w == window => Some(id),
                _ => None,
            })
    }

    #[cfg(feature="trace_layout")]
    pub fn print_tree(&self) {
        fn print(nodes: &SlotMap<NodeId, NodeData>, id: NodeId, depth: usize) {
            let indent = "  ".repeat(depth);
            match &nodes[id] {
                NodeData::Leaf { window } => tracing::info!("{indent}- Leaf: {:?}", window.get_id()),
                NodeData::Split { left, right, .. } => {
                    tracing::info!("{indent}- Split:");
                    print(nodes, *left, depth + 1);
                    print(nodes, *right, depth + 1);
                }
            }
        }

        print(&self.nodes, self.root.unwrap(), 0);
    }
}
```

### 布局树的基本操作

#### 插入窗口

窗口插入遵循当前布局策略，分为以下三个步骤：

- 定位插入节点：根据布局规则选取目标叶子节点。默认策略为插入到当前聚焦窗口所在的叶子节点，其他策略如“平衡二叉树最短边优先”可选择几何最合适的位置。
- 计算分裂区域：读取目标节点 `rec`，根据用户设置（或自动判断长宽比）决定水平或垂直分裂方式，计算新窗口的布局信息。
- 更新树结构：使用 `SlotMap` 插入一个新的 `Split` 节点，其左子节点指向原叶子节点，右子节点为新窗口节点。原节点被替换为该 `Split`，完成结构更新。

> 默认约定：左子节点为原窗口，右子节点为新插入窗口。

```rust
impl TiledTree {
    pub fn insert_window(&mut self, target: &Window, new_window: Window) -> bool {
        if let Some(target_id) = self.find_node(target) {
            // resize
            let rec = target.get_rec().unwrap();
            let (direction, l_rec, r_rec) = get_new_rec(&rec);
            target.set_rec(l_rec);
            new_window.set_rec(r_rec);

            // adjust tree
            let original = self.nodes[target_id].clone();
            let new_leaf = self.nodes.insert(NodeData::Leaf { window: new_window });
            let old_leaf = match original {
                NodeData::Leaf { window } => self.nodes.insert(NodeData::Leaf { window }),
                _ => return false,
            };

            self.nodes[target_id] = NodeData::Split {
                direction,
                rec,
                offset: (0, 0),
                left: old_leaf,
                right: new_leaf,
            };
            true
        } else {
            false
        }
    }
}

// 辅助函数
fn get_new_rec(rec: &Rectangle<i32, Logical>) -> (Direction, Rectangle<i32, Logical>, Rectangle<i32, Logical>) {

    let mut l_rec = *rec;
    let mut r_rec = *rec;

    let gap = (GAP as f32 * 0.5) as i32;
    
    if rec.size.h as f32 / rec.size.w as f32 > RATE {
        let half = rec.size.h / 2 - gap;
        l_rec.size.h = half;
        r_rec.size.h = half;
        r_rec.loc.y += half + GAP;
        (Direction::Vertical, l_rec, r_rec)
    } else {
        let half = rec.size.w / 2 - gap;
        l_rec.size.w = half;
        r_rec.size.w = half;
        r_rec.loc.x += half + GAP;
        (Direction::Horizontal, l_rec, r_rec)
    }
}
```

#### 删除窗口

窗口删除操作包含以下三个核心步骤：

- 查找关联节点：通过辅助函数 `find_parent_and_sibling` 定位目标窗口的父节点及其兄弟节点。
- 结构调整与继承布局：
  - 若兄弟节点为 `Leaf`，则继承父节点的 `rec` 并替代父节点位置；
  - 若兄弟节点为 `Split`，则同样继承 `rec`，替代父节点后调用 `modify` 递归更新其子节点的布局信息。
- 清理节点数据：从 `SlotMap` 中移除被删除的窗口节点，保持结构整洁。

```rust
impl TiledTree {
    fn find_parent_and_sibling(&self, target: NodeId) -> Option<(NodeId, NodeId)> {
        self.nodes.iter().find_map(|(id, data)| match data {
            NodeData::Split { left, right, .. } => {
                if *left == target {
                    Some((id, *right))
                } else if *right == target {
                    Some((id, *left))
                } else {
                    None
                }
            }
            _ => None,
        })
    }

    pub fn modify(&mut self, node_id: NodeId, rec: Rectangle<i32, Logical>) {
        // modify the child tree with new rec with direction
        match &mut self.nodes[node_id] {
            NodeData::Leaf { window } => {
                window.set_rec(rec);
            },
            NodeData::Split { left, right, direction, rec: current_rec, offset } => {
                let (l_rec, r_rec) = recover_new_rec(rec, direction, offset.clone());
                
                *current_rec = rec.clone();

                let left_id = *left;
                let right_id = *right;

                self.modify(left_id, l_rec);
                self.modify(right_id, r_rec);
            }
        }
    }

    pub fn remove(&mut self, target: &Window) -> bool {
        let target_id = self.find_node(target).unwrap();

        // remove last node
        if let Some(root_id) = self.root {
            if target_id == root_id {
                if let NodeData::Leaf { .. } = self.nodes[target_id] {
                    self.nodes.remove(target_id);
                    self.root = None;
                    return true;
                }
            }
        }

        let (parent_id, sibling_id) = self.find_parent_and_sibling(target_id).unwrap();

        match self.nodes[parent_id] {
            NodeData::Split { rec, .. } => {
                let sibling_data = self.nodes.remove(sibling_id).unwrap();

                match sibling_data {
                    NodeData::Leaf { window } => {
                        window.set_rec(rec.clone());
                        self.nodes[parent_id] = NodeData::Leaf { window };
                    },
                    NodeData::Split { direction, left, right, .. } => {
                        self.nodes[parent_id] = NodeData::Split { 
                            direction, 
                            rec, // from parent
                            offset: (0, 0),
                            left, 
                            right,
                        };
                        self.modify(parent_id, rec);
                    }
                }

                self.nodes.remove(target_id);

                true
            },
            NodeData::Leaf { .. } => { 
                false 
            }
        }
    }
}
```



```rust
// 辅助函数
fn recover_new_rec(rec: Rectangle<i32, Logical>, direction: &Direction, offset: (i32, i32)) -> (Rectangle<i32, Logical>, Rectangle<i32, Logical>) {
    let mut l_rec = rec;
    let mut r_rec = rec;

    let gap = (GAP as f32 * 0.5) as i32;

    match direction {
        Direction::Horizontal => {
            let half = rec.size.w / 2 - gap;
            l_rec.size.w = half;
            r_rec.size.w = half;
            r_rec.loc.x += half + GAP;

            // adjust the offset
            l_rec.size.w += offset.0;
            r_rec.size.w -= offset.0;

            r_rec.loc.x += offset.0;

            (l_rec, r_rec)
        },
        Direction::Vertical => {
            let half = rec.size.h / 2 - gap;
            l_rec.size.h = half;
            r_rec.size.h = half;
            r_rec.loc.y += half + GAP;

            // adjust the offset
            l_rec.size.h += offset.1;
            r_rec.size.h -= offset.1;

            r_rec.loc.y += offset.1;

            (l_rec, r_rec)
        }
    }
}
```



#### 移动窗口

#### 更改大小

更改窗口大小只需要判断当前窗口的父节点允许变化的方向，使用 `Smithay` 自动获取鼠标移动距离，修改 `offset` 属性即可。

```rust
// tiled_tree.rs
impl TiledTree {
    pub fn resize(&mut self, target: &Window, offset: (i32, i32)) {
        let target_id = self.find_node(target).unwrap();
        if self.get_root() == Some(target_id) {
            return;
        }
        let (parent_id, _) = self.find_parent_and_sibling(target_id).unwrap();
        match &mut self.nodes[parent_id] {
            NodeData::Split { offset: current_offset, rec, .. } => {
                current_offset.0 += offset.0;
                current_offset.1 += offset.1;
                let rec = *rec;
                self.modify(parent_id, rec);
            },
            NodeData::Leaf { .. } => { }
        }
    }
}

// resize_grab.rs
impl PointerGrab<NuonuoState> for ResizeSurfaceGrab {
    fn motion(
        &mut self,
        data: &mut NuonuoState,
        handle: &mut smithay::input::pointer::PointerInnerHandle<'_, NuonuoState>,
        _focus: Option<(
            <NuonuoState as smithay::input::SeatHandler>::PointerFocus,
            smithay::utils::Point<f64, Logical>,
        )>,
        event: &smithay::input::pointer::MotionEvent,
    ) {
        handle.motion(data, None, event);

        let delta = event.location - self.last_position;
        let focused_surface = data.seat.get_keyboard().unwrap().current_focus();
        data.workspace_manager.resize(focused_surface, (delta.x as i32, delta.y as i32));

        self.last_position = event.location;
    }
}

```



#### 倒置窗口

倒置操作主要将 `Split` 类型节点的 `direction` 参数倒置，会导致 `rec` 的变化，因此还需要更新所有子节点信息。

主要分为以下两步：

- 定位：找到需要倒置的窗口的一小段树。
- 倒置：倒置 `direction` 并且使用 `modify` 递归更新兄弟节点。

```rust
// 辅助函数
fn invert_direction(direction: &Direction) -> Direction {
    match direction {
        Direction::Horizontal => Direction::Vertical,
        Direction::Vertical => Direction::Horizontal,
    }
}

impl TiledTree {
    pub fn invert_window(&mut self, target: &Window){
        let target_id = self.find_node(target).unwrap();
        let (parent_id, _) = self.find_parent_and_sibling(target_id).unwrap();
        match &mut self.nodes[parent_id] {
            NodeData::Split { direction, rec , .. } => {
                *direction = invert_direction(direction);
                let rec = *rec;
                self.modify(parent_id, rec);
            },
            NodeData::Leaf { .. } => { }
        }
    }
}
```

### 自动布局算法

本项目支持多种窗口布局策略，通过不同算法控制窗口在树结构中的插入与删除行为，满足不同用户在操作习惯与空间分配上的需求。以下为当前支持的两种核心布局策略：

#### 跟随焦点插入 / 删除窗口（Focus-Following Mode）

此模式为默认布局策略，所有窗口的插入与删除操作均围绕当前活动窗口（focus）展开：

- 插入窗口时：查找当前焦点所在的叶子节点，并将其作为插入位置。该节点将转换为 `Split` 节点，原窗口与新窗口分别成为左右子节点。
- 删除窗口时：寻找其父节点与兄弟节点，依据兄弟节点类型进行树结构调整（详见删除操作逻辑）。

该策略逻辑直观，适合以任务上下文为导向的窗口使用场景。

#### 平衡二叉树插入（Balanced Mode）

此策略试图保持布局树的平衡性，使窗口分布更均匀，避免单边过度嵌套导致的窗口压缩问题：

- 插入窗口时：遍历当前树结构，寻找深度最浅的叶子节点作为插入点，以此保持树结构的对称性与均衡性。
- 删除窗口时：遵循相同的父子结构替换逻辑，但在后续窗口重排时尽可能维持已有平衡性。

该策略适合对窗口空间分配有强烈结构性要求的用户，如编程或数据监控等使用场景。
