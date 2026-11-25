/** A four-pointed star in the diamond's edge colour with a white heart and a soft
    glow: the sparkle. Coloured rather than white, so it shows on the pale diamond. */
export function sparkleImage(edge: string): ImageData {
	const size = 64, c = size / 2;
	const ctx = document.createElement('canvas').getContext('2d')!;
	ctx.canvas.width = ctx.canvas.height = size;
	const dirs = [[0, -1], [1, 0], [0, 1], [-1, 0]];
	const star = (tip: number, waist: number) => {
		ctx.beginPath();
		ctx.moveTo(c, c - tip);
		dirs.forEach(([dx, dy], i) => {
			const [nx, ny] = dirs[(i + 1) % 4];
			// Curve in towards the middle between one tip and the next.
			ctx.quadraticCurveTo(c + (dx + nx) * waist, c + (dy + ny) * waist, c + nx * tip, c + ny * tip);
		});
		ctx.closePath();
	};
	star(28, 7);
	ctx.shadowColor = '#ffffff';
	ctx.shadowBlur = 6;
	ctx.fillStyle = edge;
	ctx.fill();
	ctx.shadowBlur = 0;
	star(13, 3);
	ctx.fillStyle = '#ffffff';
	ctx.fill();
	return ctx.getImageData(0, 0, size, size);
}

/** Facets cut into a completed country: crossing lines and a few bright glints. */
export function facetImage(line: string): ImageData {
	const size = 40;
	const ctx = document.createElement('canvas').getContext('2d')!;
	ctx.canvas.width = ctx.canvas.height = size;
	ctx.strokeStyle = line;
	ctx.lineWidth = 2;
	ctx.beginPath();
	ctx.moveTo(0, 0); ctx.lineTo(size, size);
	ctx.moveTo(size, 0); ctx.lineTo(0, size);
	ctx.moveTo(size / 2, 0); ctx.lineTo(size, size / 2); ctx.lineTo(size / 2, size); ctx.lineTo(0, size / 2); ctx.closePath();
	ctx.stroke();
	ctx.fillStyle = 'rgb(255 255 255 / 0.8)';
	ctx.beginPath();
	ctx.moveTo(size / 2, 2); ctx.lineTo(size * 0.62, size * 0.22); ctx.lineTo(size / 2, size * 0.3); ctx.closePath();
	ctx.moveTo(4, size / 2); ctx.lineTo(size * 0.16, size * 0.42); ctx.lineTo(size * 0.2, size / 2); ctx.closePath();
	ctx.fill();
	return ctx.getImageData(0, 0, size, size);
}
