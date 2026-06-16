import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-[7px] whitespace-nowrap rounded-full font-semibold cursor-pointer transition-colors select-none disabled:cursor-not-allowed",
  {
    variants: {
      variant: {
        primary:
          "bg-nexus-accent text-white font-bold shadow-[0_2px_8px_rgba(157,122,100,.32)] hover:bg-nexus-accent-hover",
        secondary:
          "bg-nexus-card border border-nexus-border2 text-[#7a6f60] hover:bg-nexus-sand",
        subtle:
          "bg-nexus-bg border border-nexus-border2 text-[#7a6f60] hover:bg-[#ece2d5]",
        danger:
          "bg-nexus-crit text-white font-bold shadow-[0_2px_8px_rgba(181,84,64,.32)] hover:bg-[#a04a38]",
      },
      size: {
        md: "px-4 py-[9px] text-[12.5px]",
        sm: "px-[13px] py-[7px] text-[12px]",
      },
    },
    defaultVariants: { variant: "secondary", size: "md" },
  },
);

export interface ButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, type = "button", ...props }, ref) => (
    <button
      ref={ref}
      type={type}
      className={cn(buttonVariants({ variant, size }), className)}
      {...props}
    />
  ),
);
Button.displayName = "Button";

const iconButtonVariants = cva(
  "inline-flex items-center justify-center flex-none rounded-full cursor-pointer transition-colors",
  {
    variants: {
      variant: {
        card: "bg-nexus-card border border-nexus-border2 text-[#8a7a68] hover:bg-nexus-sand hover:text-nexus-accent",
        subtle:
          "bg-nexus-bg border border-nexus-border2 text-[#8a7a68] hover:bg-[#ece2d5] hover:text-nexus-accent",
      },
    },
    defaultVariants: { variant: "card" },
  },
);

export interface IconButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof iconButtonVariants> {
  /** Square size in px (default 30). */
  dim?: number;
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(
  ({ className, variant, dim = 30, type = "button", style, ...props }, ref) => (
    <button
      ref={ref}
      type={type}
      style={{ width: dim, height: dim, ...style }}
      className={cn(iconButtonVariants({ variant }), className)}
      {...props}
    />
  ),
);
IconButton.displayName = "IconButton";
