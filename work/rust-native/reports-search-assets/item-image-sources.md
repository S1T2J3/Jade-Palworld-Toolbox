# 11 个缺图物品的联网核对结果

核对日期：2026-11-12。**本范围找到 0 张可确认对应精确物品 ID 的原图；未改动软件数据、代码或版本。**

`sources.json` 为最终机器可读清单，逐项包含精确 ID、本地名称、原资料页 URL、页文件 SHA256、来源 Code、IconName、精确表格卡片 SHA256、接受图片 URL/SHA256（本次均为 null）和未采用原因。

## 直接证据

已联网保存 [PalDB Items_Table](https://paldb.cc/en/Items_Table) 完整 HTML，再按独立卡片内精确 ID 匹配。11 个 ID 均各命中一张卡片；11 张卡片的图片容器均为空。没有通过前后邻近条目的图片猜测对应关系。原始卡片保存在 `pages/exact-item-table-rows.json`。

| 精确 ID | 对应 PalDB 资料页 | 核对结果 |
|---|---|---|
| Shield_05 | [PV_ITEMS](https://paldb.cc/en/PV_ITEMS) | Code 为 Shield_05，IconName 为 `-`。不能与 Code 为 Shield_Ultra 的现用 Ultra Shield 混为一件物品。 |
| Premium_Processed_Wood | [Mythical Wooden Board](https://paldb.cc/en/Mythical_Wooden_Board) | 名称与 Code 对应，但主图为通用问号。Paldeck、GameVault、palworld.tools 也未提供真实主图。 |
| PalDopingShot_3 | [精确 ID 详情](https://paldb.cc/en/PalDopingShot_3) | Code 对应，但没有实际图标映射；不能拿 `_2` 的图替代。 |
| Potage | [Potage](https://paldb.cc/en/Potage) | 主图为通用问号；独立资料站也将其标为废弃/未启用内容。 |
| MonsterEquipWeapon_Dummy | [精确 ID 详情](https://paldb.cc/en/MonsterEquipWeapon_Dummy) | Code 对应，表格无图片，资料页为问号。 |
| MateItem01 | [精确 ID 详情](https://paldb.cc/en/MateItem01) | Code 对应，表格无图片，资料页为问号。 |
| Propellant | [同名“-”页面](https://paldb.cc/en/-) | 已定位其中 Code=Propellant 的独立记录；没有误取第一个 Gasoline 记录。表格精确卡片无图。 |
| TEST_CaptureItemModifier | [精确 ID 详情](https://paldb.cc/en/TEST_CaptureItemModifier) | Code 对应，测试记录，表格无图片，资料页为问号。 |
| FishingRod_Test | [精确 ID 详情](https://paldb.cc/en/FishingRod_Test) | Code 对应，测试记录。Paldeck 的同 ID 页面主图也为问号。 |
| SkyLightBullet | [Sunreach Rapid-Fire Ammo](https://paldb.cc/en/Sunreach_Rapid-Fire_Ammo) | 描述标 WIP。Paldeck 同 ID 页面为问号，不能借用相邻 Sky 武器的专用弹药图。 |
| SkyHeavyBullet | [Sunreach Single-Shot Ammo](https://paldb.cc/en/Sunreach_Single-Shot_Ammo) | 描述标 WIP。Paldeck 同 ID 页面为问号，不能借用相邻 Sky 武器的专用弹药图。 |

## 已排除的假匹配

- [OP.GG Potage](https://op.gg/palworld/items/potage) 的图片虽然写 `alt="Potage"`，但原始 HTML 的 `src` 实际为 `images/icons/Stone.png`，是石头，不是浓汤。
- Paldeck 与 PalDB 的 `T_icon_unknown.webp` 是问号占位素材；文件名、下载地址、SHA256 见 `rejected/manifest.json`。已实际打开三张排除对象并查看 `rejected-contact-sheet.png`，没有将它们作为物品原图。
- 原生数据提取项目 [palworld-copilot 的图标管线](https://github.com/evnja-io/palworld-copilot/blob/1620b04ee36f57411411b5a7e23ae40b39cd8b6b/packages/pipeline/src/icons.ts) 手工将神秘木板 alias 为高级木板；另一个项目把 `_3` 强化枪指向 `_2`。这些是其它物品的替代图，不满足精确原图要求。额外调查见 `research-extra/three-item-evidence.json`。

`rejected/` 中的三个下载文件**仅用于证据，不可接入应用图鉴**。图片权属未因本次下载改变；本次未做 AI 重绘。

此结果的边界是“在以上已核实的公开资料与提取项目中，未找到可确认的原图”，不等同于断言任何游戏版本或私人提取中都绝不存在该素材。
