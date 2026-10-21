import { cva, type VariantProps } from "class-variance-authority";

export const buttonVariants = cva(
  "ui-button inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-lg text-[12px] font-medium transition-[transform,box-shadow,background-color,border-color] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 active:translate-y-px",
  {
    variants: {
      variant: {
        default:
          "border border-primary/40 bg-[linear-gradient(180deg,var(--primary),color-mix(in_oklab,var(--primary)_76%,#000))] text-primary-foreground shadow-[0_7px_18px_rgba(88,101,242,0.24),inset_0_1px_0_rgba(255,255,255,0.28)] hover:brightness-110",
        secondary:
          "border border-border bg-secondary text-foreground shadow-[inset_0_1px_0_rgba(255,255,255,0.06)] hover:bg-muted",
        outline: "border border-border bg-transparent text-foreground hover:bg-muted",
        ghost: "text-muted-foreground hover:bg-muted hover:text-foreground",
        destructive:
          "border border-red-500/40 bg-[linear-gradient(180deg,#ef4b51,#c62c35)] text-white shadow-[0_7px_18px_rgba(242,63,67,0.2),inset_0_1px_0_rgba(255,255,255,0.22)] hover:brightness-110",
      },
      size: {
        default: "h-9 px-4",
        sm: "h-8 px-3 text-[11px]",
        lg: "h-10 px-5",
        icon: "h-9 w-9",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export type ButtonVariants = VariantProps<typeof buttonVariants>;
