## 窗口布局管理数据结构

- [窗口布局管理数据结构](#窗口布局管理数据结构)
  - [引言](#引言)
  - [1. 数据结构设计](#1-数据结构设计)
    - [1.1 布局方案枚举](#11-布局方案枚举)
    - [1.2 节点信息设计](#12-节点信息设计)
    - [1.3 SlotMap](#13-slotmap)
    - [1.4 全局窗口邻接表](#14-全局窗口邻接表)
  - [2. 自动布局算法](#2-自动布局算法)
    - [2.1 焦点分布（Focus-Following Mode）](#21-焦点分布focus-following-mode)
    - [2.2 网格分布（Grid Mode）](#22-网格分布grid-mode)
    - [2.3 螺旋分布（Sprial Mode）](#23-螺旋分布sprial-mode)
  - [3. 布局树的基本操作](#3-布局树的基本操作)
    - [3.1 插入窗口](#31-插入窗口)
    - [3.2 删除窗口](#32-删除窗口)
    - [3.3 倒置窗口](#33-倒置窗口)
    - [3.4 列拓展与恢复](#34-列拓展与恢复)

### 引言

在一个窗口管理器中，布局系统扮演着核心角色。为了高效管理窗口的空间排布，本项目采用了一种结构清晰、修改高效的 ***容器式二叉树（Contain Binary Tree）*** 结构作为窗口布局的基础数据模型。该树结构基于 `SlotMap` 构建，结合唯一键值索引（Key-based access），理论上可以将常规操作如插入、删除、定位的时间复杂度优化至常数级别 `O(1)`，由于窗口数量一般不超过两位数，本项目综合考量时间与空间复杂度，最终实现 `O(n)` 时间复杂度。

由于二叉树的方向表达能力不足，本项目还引入了 ***全局窗口邻接表*** 作为补充描述数据结构，记录全局所有窗口的临接方向关系。

为了管理和组织当前活动窗口的空间结构，在 `Workspace` 结构体中维护了两个核心字段：

```rust
#[derive(Debug)]
pub struct Workspace {
    ...
    pub scheme: TiledScheme,
    pub tiled_tree: Option<TiledTree>,
}
```

- `scheme`：用于指示当前使用的窗口排列方案。
- `tiled_tree`：存储当前工作区内窗口的具体排布信息，其核心数据结构即为 `TiledTree`。

### 1. 数据结构设计

#### 1.1 布局方案枚举

为布局方案定义枚举类型，默认跟随鼠标焦点布局方案。

```rust
#[derive(Debug, Clone)]
pub enum TiledScheme {
    Default,
    Spiral,
}
```

#### 1.2 节点信息设计

```rust
pub enum Direction {
    Left,
    Up,
    Right,
    Down,
}

pub enum NodeData {
    Leaf { window: Window },
    Split {
        direction: Direction,
        rec: Rectangle<i32, Logical>,
        offset: Point<i32, Logical>,
        left: NodeId,
        right: NodeId,
    }
}
```

- **内部节点（父节点）**：代表一个区域被划分的逻辑，存储以下信息：
  
  - 分裂方向：上下左右，当窗口新建时，方向被插入窗口的相对位置
  - 窗口的起点位置与总大小
  - 子节点的索引（左子节点与右子节点）
  - offset：内部分裂的偏差值（用于手动更新窗口大小）

- **叶子节点**：表示一个具体窗口的存在，只存储窗口 ID（或 Surface ID），不再包含其他布局信息。

> 说明：每次添加新窗口时，目标叶子节点会被替换为一个新的父节点，并且规定左子节点处于布局的上方/左侧，其两个子节点分别为原窗口和新窗口的 ID。

#### 1.3 SlotMap

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
    spiral_node: Option<NodeId>,
    root: Option<NodeId>,
    neighbor_graph: NeighborGraph,

    gap: i32,
}
```

在这个结构中：

- `nodes`：维护了整个布局树中所有节点的数据。
- `root`：指向当前布局树的根节点，如果树为空，则为 `None`。
- `spiral_node`：与螺旋布局有关，记录螺旋部剧的尾部。
- `neighbor_graph`：全局邻接表。
- `gap`：样式设置信息，窗口间距。

<div align = center>
    <img src = "layout/slotmap.png">
    <p style="font-size:14px;">Figure 1 slotmap</p>
</div>

以下是创建树与一些必要的工具函数：

```rust
impl TiledTree {
    pub fn new(window: Window, gap: i32) -> Self {
        let mut nodes = SlotMap::with_key();
        let root = Some(nodes.insert(NodeData::Leaf { window }));
        let spiral_node = root.clone();

        Self { 
            nodes,
            spiral_node,
            root,
            neighbor_graph: NeighborGraph::new(),

            gap,
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
    
        pub fn modify(&mut self, node_id: NodeId, rec: Rectangle<i32, Logical>, space: &mut Space<Window>, loop_handle: &LoopHandle<'_, GlobalData>) {
        // modify the child tree with new rec with direction
        match &mut self.nodes[node_id] {
            NodeData::Leaf { window } => {
                let from = space.element_geometry(&window).unwrap();

                window.set_rec(rec.size);
                space.map_element(window.clone(), rec.loc, false);

                let window = window.clone();
                loop_handle.insert_idle(move |data| {
                    data.render_manager.add_animation(
                        window,
                        from,
                        rec,
                        Duration::from_millis(30),
                        crate::animation::AnimationType::EaseInOutQuad,
                    );
                });
            },
            NodeData::Split { left, right, direction, rec: current_rec, offset } => {
                let (l_rec, r_rec) = recover_new_rec(rec, direction, offset.clone(), self.gap);

                *current_rec = rec.clone();

                let left_id = *left;
                let right_id = *right;

                self.modify(left_id, l_rec, space, loop_handle);
                self.modify(right_id, r_rec, space, loop_handle);
            }
        }
    }

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

    pub fn get_first_window(&self) -> Option<&Window> {
        let root_id = match self.get_root() {
            Some(r) => {
                r
            }
            None => {
                warn!("Failed to get root_id");
                return None
            }
        };

        fn get_window(nodes: &SlotMap<NodeId, NodeData>, id: NodeId) -> Option<&Window> {
            match &nodes[id] {
                NodeData::Leaf { window } => Some(window),
                NodeData::Split { left, .. } => {
                    get_window(nodes, *left)
                }
            }
        }

        get_window(&self.nodes, root_id)
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

#### 1.4 全局窗口邻接表

`全局窗口邻接表` 用于记录所有窗口与邻居窗口之间的位置关系，表示为 A direction B，使用 `HashMap` 进行维护。

```rust
#[derive(Debug, Clone)]
pub struct NeighborGraph {
    edges: HashMap<Window, HashMap<Direction, Vec<Window>>>,
}
```

`全局窗口邻接表` 主要完成对新插入窗口，删除窗口，更新窗口后的所有邻接关系的更新与修改。

```rust
impl NeighborGraph {
    pub fn new() -> Self {
        Self {
            edges: HashMap::new()
        }
    }

    pub fn get(&self, window: &Window, direction: &Direction) -> Option<&Vec<Window>> {
        self.edges.get(window)?.get(direction)
    }
    
    pub fn add_window(&mut self, from: Window, direction: Direction, to: Vec<Window>) {
        self.edges.entry(from).or_default().entry(direction).or_default().extend(to);
    }

    pub fn remove_window(&mut self, from: &Window, direction: Direction, to: &Window) {
        if let Some(dir_map) = self.edges.get_mut(from) {
            if let Some(vec) = dir_map.get_mut(&direction) {
                vec.retain(|win| win != to);
                if vec.is_empty() {
                    dir_map.remove(&direction);
                }
            }
            if dir_map.is_empty() {
                self.edges.remove(from);
            }
        }
    }

    pub fn remove_direction(&mut self, target: &Window, direction: &Direction) -> Option<Vec<Window>> {
        self.edges.get_mut(target)?.remove(direction)
    }

    pub fn tiled_add(&mut self, from: Window, direction: Direction, new: Window) {
        let opposite = direction.opposite();
        let orthogonal = direction.orthogonal();

        // new <--> orthogonal neighbors
        for d in orthogonal {
            if let Some(neighbors_orthogonal) = self.get(&from, &d).cloned() {
                
                for neighbor in &neighbors_orthogonal {
                    self.add_window(neighbor.clone(), d.opposite(), vec![new.clone()]);
                }
                
                self.add_window(new.clone(), d.clone(), neighbors_orthogonal);
            }
        }

        // new <--> neighbors
        if let Some(neighbors_direction) = self.remove_direction(&from, &direction) {
            
            for neighbor in &neighbors_direction {
                self.remove_window(neighbor, opposite.clone(), &from);
                self.add_window(neighbor.clone(), opposite.clone(), vec![new.clone()]);
            }
            
            self.add_window(new.clone(), direction.clone(), neighbors_direction);
        }

        // new <--> from
        self.add_window(from.clone(), direction, vec![new.clone()]);
        self.add_window(new, opposite, vec![from]);
    }

    pub fn exchange(&mut self, a: &Window, b: &Window) {
        let a_neighbors = self.edges.remove(a).unwrap_or_default();
        let b_neighbors = self.edges.remove(b).unwrap_or_default();

        self.edges.insert(a.clone(), b_neighbors);
        self.edges.insert(b.clone(), a_neighbors);

        // exchange a <-> b
        for (_, dir_map) in self.edges.iter_mut() {
            for (_, neighbors) in dir_map.iter_mut() {
                for win in neighbors.iter_mut() {
                    if win == a {
                        *win = b.clone();
                    } else if win == b {
                        *win = a.clone();
                    }
                }
            }
        }
    }

    #[cfg(feature="trace_layout")]
    pub fn print(&self) {
        for (from, hash_map) in &self.edges {
            info!("Window {:?} connections:", from.geometry().size);
            for (direction, to_list) in hash_map {
                for to in to_list {
                    info!("  ├── {:?} -> {:?}", direction, to.geometry().size);
                }
            }
        }
    }
}
```

### 2. 自动布局算法

本项目支持多种窗口布局策略，通过不同算法控制窗口在树结构中的插入与删除行为，满足不同用户在操作习惯与空间分配上的需求。以下为当前支持的两种核心布局策略：

#### 2.1 焦点分布（Focus-Following Mode）

此模式为默认布局策略，所有窗口的插入与删除操作均围绕当前活动窗口（focus）展开：

- 插入窗口时：查找当前焦点所在的叶子节点，并将其作为插入位置。该节点将转换为 `Split` 节点，原窗口与新窗口分别成为左右子节点。
- 删除窗口时：寻找其父节点与兄弟节点，依据兄弟节点类型进行树结构调整（详见删除操作逻辑）。

<div align = center>
    <img src = "layout/focus.png">
    <p style="font-size:14px;">Figure 2 Focus-Following Mode</p>
</div>

#### 2.2 网格分布（Grid Mode）

此策略试图保持布局树的平衡性，使窗口分布更均匀，避免单边过度嵌套导致的窗口压缩问题：

- 插入窗口时：遍历当前树结构，寻找深度最浅的叶子节点作为插入点，以此保持树结构的对称性与均衡性。
- 删除窗口时：遵循相同的父子结构替换逻辑，但在后续窗口重排时尽可能维持已有平衡性。

<div align = center>
    <img src = "layout/grid.png">
    <p style="font-size:14px;">Figure 3 Grid Mode</p>
</div>

#### 2.3 螺旋分布（Sprial Mode）

此策略试图实现螺旋状的窗口分布，以左侧为起始，实现动态美观的布局效果。

- 插入窗口时：记录的 `sprial_node` 为插入节点，插入方向为*右下左上*轮换，按照当前窗口的数量计算得到。
- 删除窗口时：若删除窗口为 `sprial_node` 则设置其兄弟节点为新的 `sprial_node`。

<div align = center>
    <img src = "layout/sprial.png">
    <p style="font-size:14px;">Figure 4 Sprial Mode</p>
</div>

### 3. 布局树的基本操作

#### 3.1 插入窗口

窗口插入遵循当前布局策略，分为以下三个步骤：

1. 确定被插入窗口与插入方向
2. 计算与设置被插入窗口与新插入窗口的大小与位置
3. 修改邻接表

`workspace` 根据布局策略，给定被插入窗口与插入方向，`insert_window()` 函数会完成计算与更新修改，这里的设计非常符合直觉。

```rust
impl TiledTree {
        pub fn insert_window(
        &mut self, 
        target: Option<&Window>, 
        new_window: Window, 
        direction: Direction, 
        space: &mut Space<Window>,
        loop_handle: &LoopHandle<'_, GlobalData>,
    ) -> bool {

        let target = match target {
            Some(window) => window.clone(),
            None => {
                match self.get_first_window() {
                    Some(window) => window.clone(),
                    None => {
                        warn!("Failed to get first window");
                        return false
                    }
                }
            }
        };

        if let Some(target_id) = self.find_node(&target) {
            // resize
            // TODO: use server geometry
            let rec = match space.element_geometry(&target){
                Some(r) => r,
                None => {
                    warn!("Failed to get window rectangle");
                    return false
                }
            };
            
            let mut original_rec = rec.clone();
            let new_rec = get_new_rec(&direction, &mut original_rec, self.gap);
            
            // TODO: merge
            target.set_rec(original_rec.size);
            new_window.set_rec(new_rec.size);
            space.map_element(target.clone(), original_rec.loc, false);
            space.map_element(new_window.clone(), new_rec.loc, true);

            // adjust tree
            let old_leaf = self.nodes.insert(NodeData::Leaf { window: target.clone() });
            let new_leaf = self.nodes.insert(NodeData::Leaf { window: new_window.clone() });

            self.spiral_node = Some(new_leaf);

            // use split node hold leafs
            match direction {
                Direction::Left | Direction::Up => {
                    self.nodes[target_id] = NodeData::Split {
                        direction: direction.clone(),
                        rec,
                        offset: (0, 0).into(),
                        left: new_leaf,
                        right: old_leaf,
                    };
                }
                _ => {
                    self.nodes[target_id] = NodeData::Split {
                        direction: direction.clone(),
                        rec,
                        offset: (0, 0).into(),
                        left: old_leaf,
                        right: new_leaf,
                    };
                }   
            }

            // modify neighbor_graph
            self.neighbor_graph.tiled_add(target.clone(), direction.clone(), new_window.clone());

            // TODO: use config
            // create animation
            loop_handle.insert_idle(move |data| {
                data.render_manager.add_animation(
                    target,
                    rec,
                    original_rec,
                    Duration::from_millis(30),
                    crate::animation::AnimationType::EaseInOutQuad,
                );

                let mut from = new_rec;
                match direction {
                    Direction::Right => {
                        from.loc.x += from.size.w;
                    }
                    Direction::Left => {
                        from.loc.x -= from.size.w;
                    }
                    Direction::Up => {
                        from.loc.y -= from.size.h;
                    }
                    Direction::Down => {
                        from.loc.y += from.size.h;
                    }
                }

                data.render_manager.add_animation(
                    new_window,
                    from,
                    new_rec,
                    Duration::from_millis(30),
                    crate::animation::AnimationType::EaseInOutQuad,
                );
            });

            true
        } else {
            false
        }
    }
}

// 工具函数，用于计算新旧窗口的大小与位置
fn get_new_rec(direction: &Direction, rec: &mut Rectangle<i32, Logical>, gap: i32) -> Rectangle<i32, Logical> {

    let mut new_rec = *rec;

    match direction {
        Direction::Left | Direction::Right => {
            let half = rec.size.w / 2 - gap;
            new_rec.size.w = half;
            rec.size.w = half;

            if direction == &Direction::Left {
                rec.loc.x += half + gap;
            } else {
                new_rec.loc.x += half + gap;
            }

            new_rec
        }
        Direction::Up | Direction::Down => {
            let half = rec.size.h / 2 - gap;
            new_rec.size.h = half;
            rec.size.h = half;

            if direction == &Direction::Up {
                rec.loc.y += half + gap;
            } else {
                new_rec.loc.y += half + gap;
            }

            new_rec
        }
    }
}
```

#### 3.2 删除窗口

窗口删除操作包含以下四个核心步骤：

- 查找关联节点：通过辅助函数 `find_parent_and_sibling` 定位目标窗口的父节点及其兄弟节点。
- 结构调整与继承布局：
  - 若兄弟节点为 `Leaf`，则继承父节点的 `rec` 并替代父节点位置；
  - 若兄弟节点为 `Split`，则同样继承 `rec`，替代父节点后调用 `modify` 递归更新其子节点的布局信息。
- 清理节点数据：从 `SlotMap` 中移除被删除的窗口节点，保持结构整洁。
- 更新邻接表

```rust
impl TiledTree {

    pub fn remove(&mut self, target: &Window, focus: &mut Option<Window>, space: &mut Space<Window>, loop_handle: &LoopHandle<'_, GlobalData>) -> bool {
        let target_id = match self.find_node(target) {
            Some(r) => {
                r
            }
            None => {
                warn!("Failed to get target_id");
                return false
            }
        };

        // remove last node
        if Some(target_id) == self.root {
            if matches!(self.nodes[target_id], NodeData::Leaf { .. }) {
                self.nodes.remove(target_id);
                self.root = None;
                *focus = None;
                return true;
            }
        }

        let (parent_id, sibling_id) = match self.find_parent_and_sibling(target_id) {
            Some(r) => r,
            None => {
                warn!("Failed to get node: {:?} parent and sibling", target_id);
                return false
            }
        };

        if self.spiral_node == Some(target_id) {
            self.spiral_node = Some(parent_id);
        }

        match self.nodes[parent_id] {
            NodeData::Split { rec, .. } => {
                let sibling_data = match self.nodes.remove(sibling_id){
                    Some(r) => r,
                    None => {
                        warn!("Failed to remove sibling: {:?}", sibling_id);
                        return false
                    }
                };

                match sibling_data {
                    NodeData::Leaf { window } => {
                        let from = space.element_geometry(&window).unwrap();

                        window.set_rec(rec.size);
                        space.map_element(window.clone(), rec.loc, false);

                        self.nodes[parent_id] = NodeData::Leaf { window: window.clone() };

                        if focus.as_ref() == Some(target) {
                            *focus = Some(window.clone());
                        }

                        loop_handle.insert_idle(move |data| {
                            data.render_manager.add_animation(
                                window,
                                from,
                                rec,
                                Duration::from_millis(30),
                                crate::animation::AnimationType::EaseInOutQuad,
                            );
                        });
                    },
                    NodeData::Split { direction, left, right, .. } => {
                        self.nodes[parent_id] = NodeData::Split { 
                            direction, 
                            rec, // from parent
                            offset: (0, 0).into(),
                            left, 
                            right,
                        };
                        self.modify(parent_id, rec, space, loop_handle);

                        if focus.as_ref() == Some(target) {
                            *focus = self.get_first_window().cloned();
                        }

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

// 工具函数，复原被删除窗口的兄弟窗口
fn recover_new_rec(rec: Rectangle<i32, Logical>, direction: &Direction, offset: Point<i32, Logical>, gap: i32) -> (Rectangle<i32, Logical>, Rectangle<i32, Logical>) {
    let mut l_rec = rec;
    let mut r_rec = rec;

    match direction {
        Direction::Left | Direction::Right => {
            let half = rec.size.w / 2 - gap;
            l_rec.size.w = half + offset.x;
            r_rec.size.w = half - offset.x;

            r_rec.loc.x += half + gap + offset.x;
        }
        Direction::Up | Direction::Down => {
            let half = rec.size.h / 2 - gap;
            l_rec.size.h = half + offset.y;
            r_rec.size.h = half - offset.y;

            r_rec.loc.y += half + gap + offset.y;
        }
    }

    (l_rec, r_rec)
}
```

#### 3.3 倒置窗口

倒置操作主要将 `Split` 类型节点的 `direction` 参数倒置，视觉效果上为水平变竖直，此操作会导致 `rec` 的变化，因此还需要更新所有子节点信息。

主要分为以下三步：

- 找到需要倒置的窗口的父节点。
- 倒置 `direction` 并且使用 `modify` 递归更新当前父元素为根的树。
- 修改邻接表。

```rust
impl TiledTree {
    pub fn invert_window(&mut self, target: &Window, space: &mut Space<Window>, loop_handle: &LoopHandle<'_, GlobalData>){
        let target_id = match self.find_node(target) {
            Some(r) => {
                r
            }
            None => {
                warn!("Failed to get target_id");
                return
            }
        };

        // Only single window
        if self.get_root() == Some(target_id) {
            return;
        }

        let (parent_id, _) = match self.find_parent_and_sibling(target_id) {
            Some(r) => r,
            None => {
                warn!("Failed to get node: {:?} parent and sibling", target_id);
                return
            }
        };

        match &mut self.nodes[parent_id] {
            NodeData::Split { direction, rec , .. } => {
                *direction = direction.rotate_cw();
                let rec = *rec;
                self.modify(parent_id, rec, space, loop_handle);
            },
            NodeData::Leaf { .. } => { }
        }
    }
}
```

#### 3.4 列拓展与恢复

此操作实现将所有窗口按照统一大小进行排布。

```rust
impl TiledTree{
    pub fn expansion(&self, space: &mut Space<Window>, loop_handle: &LoopHandle<'_, GlobalData>) {
        if let Some(bound) = self.get_root_rec(space) {
            let width = (bound.size.w - 2*self.gap) / 3;
            let height = bound.size.h;
            let mut loc = bound.loc;

            for node in self.nodes.values() {
                match node {
                    NodeData::Leaf { window } => {
                        let from = space.element_geometry(window).unwrap();

                        window.set_rec((width, height).into());
                        space.map_element(window.clone(), loc, false);

                        let window = window.clone();

                        loop_handle.insert_idle(move |data| {
                            data.render_manager.add_animation(
                                window,
                                from,
                                Rectangle { loc, size: (width, height).into() },
                                Duration::from_millis(30),
                                crate::animation::AnimationType::EaseInOutQuad,
                            );
                        });

                        loc.x = loc.x + width + self.gap;
                    }
                    _ => { }
                }
            }
        }
    }

    pub fn recover(&mut self, space: &mut Space<Window>, loop_handle: &LoopHandle<'_, GlobalData>) {
        if let Some(root_id) = self.get_root() {
            match self.nodes[root_id] {
                NodeData::Split { rec , .. } => {
                    self.modify(root_id, rec, space, loop_handle);
                }
                _ => { }
            }
        }
    }
}
```