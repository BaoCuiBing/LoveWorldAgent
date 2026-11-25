import { clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'
// 合并类名：clsx 处理条件类名，twMerge 让后写的 Tailwind 类覆盖同类属性
export const cn = (...inputs) => twMerge(clsx(inputs))
